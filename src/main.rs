use chrono::{DateTime, Utc};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use influxdb::{Client, InfluxDbWriteable};
use pinger::{ping_with_interval, PingResult};
use serde::Deserialize;
use std::time::Duration;

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
                println!("{:?} (line: {})", duration, line);
                results.push(PingMeasurement {
                    time: Utc::now(),
                    duration: duration.as_millis().try_into().unwrap(),
                    host: host.clone(),
                });
            }
            PingResult::Timeout(_) => println!("Timeout!"),
            PingResult::Unknown(line) => println!("Unknown line: {}", line),
            PingResult::PingExited(_code, _stderr) => {}
        }
    }

    results
}

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default = "default_resource")]
    ping_count: u8,
    ping_host: Vec<String>,

    // InfluxDB parameters
    influxdb: InfluxDB,
}

fn default_resource() -> u8 {
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
    let conf: Config = Figment::new()
        .merge(Toml::file("./config.toml"))
        .extract()
        .unwrap();

    let hosts = conf.ping_host;
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
