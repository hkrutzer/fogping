use std::time::Duration;
use chrono::{DateTime, Utc};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use log::{debug, warn};
use influxdb::{Client, InfluxDbWriteable};
use pinger::{ping_with_interval, PingResult};
use serde::Deserialize;

#[derive(Clone, InfluxDbWriteable)]
struct PingMeasurement {
    time: DateTime<Utc>,
    duration: i32,
    #[influxdb(tag)]
    host: String,
}

async fn run_ping(host: String, count: u8) -> Vec<PingMeasurement> {
    let mut results = Vec::<PingMeasurement>::with_capacity(count.into());

    let stream =
        ping_with_interval(host.clone(), Duration::from_millis(300), None).expect("error pinging");

    for _ in 0..count {
        match stream.recv().unwrap() {
            PingResult::Pong(duration, line) => {
                debug!("{:?} (line: {})", duration, line);
                results.push(PingMeasurement {
                    time: Utc::now(),
                    duration: duration.as_millis().try_into().unwrap(),
                    host: host.clone(),
                });
            }
            PingResult::Timeout(_) => warn!("Timeout!"),
            PingResult::Unknown(line) => warn!("Unknown line: {}", line),
            PingResult::PingExited(_code, _stderr) => {}
        }
    }

    results
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
    env_logger::init();

    let conf: Config = Figment::new()
        .merge(Toml::file("./config.toml"))
        .extract()
        .expect("Failed to load configuration");

    let hosts = conf.ping_targets;
    let mut client = Client::new(conf.influxdb.host, conf.influxdb.db);

    if let Some(token) = conf.influxdb.token {
        client = client.with_token(token)
    }

    let mut tasks = Vec::with_capacity(hosts.len());
    for host in hosts {
        tasks.push(tokio::spawn(run_ping(host, conf.ping_count)));
    }

    for task in tasks {
        let results: Vec<influxdb::WriteQuery> = task
            .await
            .unwrap()
            .iter()
            .map(|x| x.clone().into_query("ping_measurement"))
            .collect();
        let write_result = client.query(results).await;
        assert!(write_result.is_ok(), "Influx write result was not okay");
    }
}
