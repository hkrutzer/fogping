use figment::{
    providers::{Format, Toml},
    Figment,
};
use log::{debug, error, info, warn};
use pinger::{ping_with_interval, PingResult};
use serde::Deserialize;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

mod store;
use crate::store::Store;

#[derive(Debug)]
struct Measurement {
    time: u128,
    duration: Duration,
    host: String,
}

async fn run_ping(host: String, count: u8, tx: mpsc::Sender<Measurement>) {
    let stream =
        ping_with_interval(host.clone(), Duration::from_millis(300), None).expect("error pinging");

    for message in stream.into_iter().take(count as usize) {
        match message {
            PingResult::Pong(duration, line) => {
                let time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();

                tx.send(Measurement {
                    host: host.clone(),
                    time,
                    duration,
                })
                .await
                .expect("Failed to send measurement");

                debug!("Ping duration: {:?} (line: {})", duration, line);
            }
            PingResult::Timeout(_) => warn!("Ping timeout"),
            PingResult::Unknown(line) => error!("Unknown line: {}", line),
            PingResult::PingExited(code, stderr) => {
                error!("Ping exited with code {}: {}", code, stderr);
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default = "default_ping_count")]
    ping_count: u8,
    ping_targets: Vec<String>,

    // InfluxDB parameters
    influxdb: InfluxDB,
}

fn default_ping_count() -> u8 {
    10
}

#[derive(Debug, Deserialize)]
struct InfluxDB {
    host: String,
    db: String,
    token: Option<String>,
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./config.toml".to_string());

    let conf: Config = Figment::new()
        .merge(Toml::file(&config_path))
        .extract()
        .expect("Failed to load configuration");

    let mut client = store::influxdb::InfluxDBStore::new(conf.influxdb);

    let hosts = conf.ping_targets;

    let (tx, mut rx) = mpsc::channel::<Measurement>(33);

    info!(
        "Starting pings for {} hosts with {} pings each",
        hosts.len(),
        conf.ping_count
    );

    // Spawn a task to handle storing measurements
    let store_task = tokio::spawn(async move {
        while let Some(measurement) = rx.recv().await {
            if let Err(e) = client.add_measurement(measurement).await {
                error!("Failed to add measurement: {}", e);
            }
        }

        // Flush when channel is closed
        if let Err(e) = client.flush().await {
            error!("Failed to flush: {}", e);
        }
    });

    let tasks: Vec<_> = hosts
        .into_iter()
        .map(|item| tokio::spawn(run_ping(item, conf.ping_count, tx.clone())))
        .collect();
    let _ = futures::future::join_all(tasks).await;
    drop(tx);

    store_task.await.unwrap();
}
