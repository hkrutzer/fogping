# fogping
A network monitoring tool that pings hosts and stores the results in InfluxDB.

## Usage

1. Create a `config.toml` file in your working directory
2. Run the application:

```bash
fogping
```

## Configuration

Create a `config.toml` file with the following structure:

```toml
ping_targets = [
  "1.1.1.1",
  "github.com",
]

[influxdb]
host = "https://your.influx.host"
token = "your-influxdb-token"
db = "ping"
```
