# killswitch

A lightweight HTTP API server for managing process lifecycle remotely. Start, stop, and monitor a child process through simple REST endpoints.

## Features

- Start and stop a managed child process via HTTP API
- Monitor process status and PID
- Optional token-based authentication
- Automatic process health monitoring
- Built with Rust for performance and reliability

## Installation

### From Source

```bash
cargo build --release
```

The binary will be available at `target/release/killswitch`.

## Usage

```bash
killswitch [OPTIONS] <COMMAND>...
```

### Arguments

- `<COMMAND>...` - The command and arguments to execute as the managed process (required)

### Options

- `-P, --port <PORT>` - Port to listen on (default: 9966)
- `-h, --help` - Print help information

### Examples

Start a simple Python HTTP server:
```bash
killswitch python3 -m http.server 8000
```

Listen on a custom port:
```bash
killswitch --port 3000 npm start
```

With authentication enabled:
```bash
export KILLSWITCH_PASSWORD="your-secret-token"
killswitch ./your-app --arg1 --arg2
```

## API Endpoints

### GET /

Health check endpoint.

**Response:** `Hello, world!`

### GET /status

Get the current status of the managed process.

**Response:**
- `Process is running with PID <pid>` - Process is active
- `No process is running` - Process is stopped

### POST /start

Start the managed process if it's not already running.

**Response:**
- `200 OK` - Process started successfully
- `409 Conflict` - Process is already running
- `500 Internal Server Error` - Failed to start process

### POST /stop

Stop the currently running process.

**Response:**
- `200 OK` - Process stopped successfully
- `404 Not Found` - No process is running
- `500 Internal Server Error` - Failed to stop process

## Authentication

Authentication is optional and enabled via environment variable:

```bash
export KILLSWITCH_PASSWORD="your-secret-token"
```

When enabled, all API requests must include an Authorization header:

```bash
curl -H "Authorization: Bearer your-secret-token" http://localhost:9966/status
```

Example authenticated requests:

```bash
# Check status
curl -H "Authorization: Bearer your-secret-token" http://localhost:9966/status

# Start process
curl -X POST -H "Authorization: Bearer your-secret-token" http://localhost:9966/start

# Stop process
curl -X POST -H "Authorization: Bearer your-secret-token" http://localhost:9966/stop
```

## Process Monitoring

killswitch automatically monitors the managed process:

- Initial health check after 1 second to ensure successful startup
- Periodic status checks every 5 seconds
- Automatic cleanup when process exits
- Server exits if process fails during initial startup

## Dependencies

- [axum](https://github.com/tokio-rs/axum) - Web framework
- [tokio](https://tokio.rs/) - Async runtime
- [clap](https://github.com/clap-rs/clap) - Command line argument parsing
- [tracing](https://github.com/tokio-rs/tracing) - Logging framework

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
