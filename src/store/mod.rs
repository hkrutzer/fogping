pub mod influxdb;

use crate::Measurement;

pub trait Store {
    async fn add_measurement(&mut self, measurement: Measurement) -> Result<(), String>
    where
        Self: Send;

    async fn flush(&mut self) -> Result<(), String>
    where
        Self: Send;
}
