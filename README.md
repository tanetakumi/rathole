# rathole - Simplified CLI Version

A simplified, configuration-free version of [rathole](https://github.com/rapiz1/rathole) - A secure, stable and high-performance reverse proxy for NAT traversal, written in Rust.

## About This Fork

This is a **highly simplified fork** of the original rathole project by [rapiz1](https://github.com/rapiz1). The original rathole is a full-featured, production-ready reverse proxy with advanced security features, multiple transport protocols, and extensive configuration options.

**This simplified version** focuses on ease of use with:
- ✅ **No configuration files required** - Everything through CLI arguments
- ✅ **Automatic port allocation** - Server automatically assigns ports (35100-35200)
- ✅ **Simple API** - Easy to integrate into Rust programs
- ✅ **Multiple clients support** - One server, multiple concurrent clients
- ✅ **Auto-reconnect** - Automatic reconnection on failure
- ✅ **Minimal codebase** - ~830 lines (73% reduction from original)

**⚠️ Important Notes:**
- **No authentication** - Not suitable for production use
- **TCP only** - UDP support removed
- **Breaking changes** - Not compatible with original rathole protocol
- **Educational/Development use** - Best for local networks and development

## Original rathole

The original [rathole](https://github.com/rapiz1/rathole) by rapiz1 provides:
- Token-based authentication for security
- Multiple transport protocols (TCP, TLS, Noise, WebSocket)
- Hot-reload configuration
- Production-grade performance and stability
- Comprehensive documentation and examples

👉 **For production use, please use the [original rathole](https://github.com/rapiz1/rathole)**

## Installation

```bash
cargo build --release
```

The binary will be available at `target/release/rathole`.

## Quick Start

### Server Mode

```bash
# Start server (default: 0.0.0.0:2333)
rathole server

# Or specify custom address
rathole server 0.0.0.0:8080
```

### Client Mode

```bash
# Expose local port 8080 through remote server
rathole client myserver.com:2333 8080

# Example output:
# Tunnel established! Remote port: 35100
# Press Ctrl+C to stop...
```

Now access `myserver.com:35100` to reach your local service at `127.0.0.1:8080`.

## Usage Examples

### Example 1: Expose Local Web Server

```bash
# Server side (e.g., VPS)
rathole server 0.0.0.0:2333

# Client side (e.g., home PC running web server on port 8080)
rathole client vps.example.com:2333 8080
# → Assigned remote port: 35100

# Access your local web server at: vps.example.com:35100
```

### Example 2: Expose SSH Service

```bash
# Client side
rathole client myserver.com:2333 22
# → Assigned remote port: 35100

# Connect from anywhere
ssh user@myserver.com -p 35100
```

### Example 3: Multiple Services Simultaneously

```bash
# Client 1: Web server
rathole client myserver.com:2333 8080
# → Remote port: 35100

# Client 2: SSH
rathole client myserver.com:2333 22
# → Remote port: 35101

# Client 3: Database
rathole client myserver.com:2333 5432
# → Remote port: 35102
```

All clients can run simultaneously, each getting a unique port.

## Use as a Library

```rust
use rathole::start_tunnel;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Start tunnel
    let tunnel = start_tunnel("myserver.com:2333", 8080).await?;

    println!("Tunnel established!");
    println!("Remote port: {}", tunnel.remote_port());
    println!("Access via: myserver.com:{}", tunnel.remote_port());

    // Keep tunnel alive
    tokio::signal::ctrl_c().await?;

    // Graceful shutdown
    tunnel.shutdown().await?;

    Ok(())
}
```

## Java Client

This version uses **JSON protocol** which allows clients in other languages!

A Java client implementation is available in `clients/java/`:

```bash
# Build Java client
cd clients/java
mvn clean package

# Run Java client
java -jar target/rathole-client.jar myserver.com 2333 8080
```

```java
// Use as library in your Java project
RatholeClient client = new RatholeClient("myserver.com", 2333, 8080);
client.start();
System.out.println("Remote port: " + client.getAssignedPort());
```

See [clients/java/README.md](clients/java/README.md) for detailed documentation.

**Why JSON?** The protocol now uses JSON instead of Rust-specific bincode, making it easy to implement clients in any language (Python, Go, Node.js, etc.).

## Features

### What's Included
- Simple CLI with no configuration files
- **JSON protocol** - Language-independent, easy to implement in any language
- Automatic port allocation (35100-35200 range)
- Support for up to 100 concurrent clients
- Automatic reconnection on failure
- Heartbeat for connection health monitoring
- Clean and readable codebase
- **Java client included** - Full-featured Java implementation

### What's Removed (from original rathole)
- TOML configuration system
- Token-based authentication
- TLS/Noise/WebSocket transports (TCP only)
- UDP support
- Hot-reload functionality
- Service management features
- Connection pooling optimization

## Port Range

The server automatically allocates ports in the range **35100-35200**.

Maximum 100 clients can connect simultaneously.

## Logging

Control log level with `RUST_LOG` environment variable:

```bash
# Debug logs
RUST_LOG=debug rathole server

# Errors only
RUST_LOG=error rathole client myserver.com:2333 8080

# Per-module logging
RUST_LOG=rathole=debug,tokio=info rathole server
```

## Troubleshooting

### Cannot Connect
1. Check if server port (2333) is open
2. Check firewall for port range 35100-35200
3. Enable debug logs: `RUST_LOG=debug`

### Port Exhaustion
If more than 100 clients try to connect, new connections will fail. Disconnect existing clients first.

### Cannot Connect to Local Service
Ensure the local service is actually running on the specified port (e.g., 8080).

## Comparison

| Feature | Original rathole | This Simplified Version |
|---------|-----------------|-------------------------|
| Configuration | TOML files | CLI arguments only |
| Authentication | Token-based | None |
| Port assignment | Manual | Automatic |
| Transport | TCP/TLS/Noise/WebSocket | TCP only |
| Protocol | TCP/UDP | TCP only |
| Lines of code | ~3,147 | ~830 |
| Production ready | ✅ Yes | ❌ No |
| Security | ✅ High | ❌ None |

## Architecture

```
src/
├── main.rs           (82 lines)  - CLI entry point
├── lib.rs            (13 lines)  - Public API
├── protocol.rs       (104 lines) - Simple protocol (4 message types)
├── port_allocator.rs (100 lines) - Port management
├── client.rs         (220 lines) - Client implementation
├── server.rs         (295 lines) - Server implementation
└── tunnel.rs         (88 lines)  - Tunnel API
```

## Security Warning

⚠️ **This version has NO authentication mechanisms**

- Only use in trusted networks
- Not recommended for Internet-facing deployments
- For production, use the [original rathole](https://github.com/rapiz1/rathole) with proper security configuration
- Consider using VPN or SSH tunneling for additional security

## Documentation

See [USAGE_SIMPLE.md](./USAGE_SIMPLE.md) for detailed usage guide.

## Credits

This project is a simplified fork of [rathole](https://github.com/rapiz1/rathole) by [rapiz1](https://github.com/rapiz1).

The original rathole is licensed under Apache License 2.0 and provides a production-ready, secure, and feature-rich reverse proxy solution.

**All credit for the original design, architecture, and implementation goes to the original rathole project and its contributors.**

## License

Apache License 2.0

This simplified version maintains the same Apache License 2.0 as the original rathole project.

See [LICENSE](./LICENSE) for full license text.

## Links

- **Original rathole**: https://github.com/rapiz1/rathole
- **Original rathole documentation**: https://github.com/rapiz1/rathole#readme
- **This simplified version**: Designed for educational and development purposes only

---

**Note**: If you need a production-ready reverse proxy with security features, please use the [original rathole](https://github.com/rapiz1/rathole) instead of this simplified version.
