use crate::{store::Store, Measurement};
use influxdb::{Client, InfluxDbWriteable, Timestamp, WriteQuery};

pub struct InfluxDBStore {
    client: Client,
    data: Vec<WriteQuery>,
}

impl Store for InfluxDBStore {
    async fn add_measurement(&mut self, data: Measurement) -> Result<(), String> {
        let query = Timestamp::Nanoseconds(data.time)
            .into_query("ping_measurement")
            .add_field("duration", data.duration.as_millis() as u64)
            .add_tag("target", data.host)
            .add_tag(
                "from",
                // TODO Make this configurable
                hostname::get()
                    .expect("Failed to get hostname")
                    .to_string_lossy()
                    .to_string(),
            );

        self.data.push(query);
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), String> {
        if self.data.is_empty() {
            return Ok(());
        }

        let write_result = self.client.query(&self.data).await;
        self.data.clear();
        match write_result {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to write data to InfluxDB: {}", e)),
        }
    }
}

impl InfluxDBStore {
    pub fn new(config: crate::InfluxDB) -> Self {
        let mut client = Client::new(config.host, config.db);

        if let Some(token) = config.token {
            client = client.with_token(token)
        }

        InfluxDBStore {
            client,
            data: Vec::new(),
        }
    }
}
