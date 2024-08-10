# rgate (Rusty Gate)

`rgate (Rusty Gate)` is a simple HTTP proxy and logging tool written in Rust. It proxies requests to a specified base URL and logs the details of each request and response. It also provides a web dashboard to view the logged requests in real-time.

## Features

- Proxies HTTP requests to a specified base URL
- Logs details of each request and response, including headers, body, and response time
- Provides a web dashboard to view logged requests in real-time
- Supports WebSocket connections to stream log entries to the dashboard
- CLI with configurable options

## Installation

### Building from Source
1. Clone the repository:

```bash
git clone https://github.com/yourusername/rgate.git
cd rgate

```

### Build the project:
```bash
cargo build --release
```

### Install the executable:
```bash
cargo install --path .
```

### Running with Docker
You can also run `rgate` using Docker:

```bash
docker build -t rgate .
docker run -p 9000:9000 -p 9001:9001 rgate https://example.com
```

## Usage

### Basic Usage
After installing, you can use the `rgate` command as follows:

```bash
rgate <URL> [OPTIONS]
```

### Example
To proxy requests to `https://example.com` and listen on default ports `9000` for the proxy and `9001` for the dashboard:

```bash
rgate https://example.com
```

To proxy requests to `https://other-example.com` and listen on port `9100` for the proxy and `9101` for the dashboard:

```bash
rgate https://other-example.com --port 9100 --dashboard-port 9101
```

### Options
- `<URL>`: The base URL to which requests will be proxied.
- `--port <PORT>`: The port on which the proxy server will listen. Defaults to 9000.
- `--dashboard-port <PORT>`: The port on which the dashboard will listen. Defaults to 9001.

### Example Output
When the application starts, you'll see a message like:

```plaintext
Proxying https://example.com on http://localhost:9000
```

As requests come in, you'll see log messages like:

```plaintext
GET /path/to/resource 123ms 200
```

### Web Dashboard
Access the web dashboard at `http://localhost:9001/dashboard`. The dashboard provides a real-time view of the requests being proxied.

### Clear Logs
You can clear the logs by clicking the "Clear" button on the dashboard. This will clear the logs from both the web interface and the backend.

### License
This project is licensed under the MIT License. See the [LICENSE](./LICENSE) file for details.