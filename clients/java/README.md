# Rathole Java Client

Java client implementation for [Rathole](https://github.com/rapiz1/rathole) reverse proxy.

This client connects to a Rathole server (written in Rust) and establishes a tunnel to expose a local service through the server.

## Features

- ✅ JSON-based protocol (language-independent)
- ✅ Automatic reconnection
- ✅ Heartbeat mechanism
- ✅ Multiple data channels support
- ✅ Simple API

## Requirements

- Java 11 or higher
- Maven or Gradle (for building)

## Building

### Using Maven

```bash
cd clients/java
mvn clean package
```

This will create `target/rathole-client.jar` (fat JAR with all dependencies).

### Using Java directly (without Maven)

```bash
cd clients/java/src/main/java

# Compile (need gson jar)
javac -cp .:gson-2.10.1.jar com/rathole/RatholeClient.java

# Run
java -cp .:gson-2.10.1.jar com.rathole.RatholeClient <server_addr> <server_port> <local_port>
```

## Usage

### Command Line

```bash
# Using the fat JAR
java -jar target/rathole-client.jar <server_addr> <server_port> <local_port>

# Example: Expose local port 8080 through server at localhost:2333
java -jar target/rathole-client.jar localhost 2333 8080
```

### As a Library

```java
import com.rathole.RatholeClient;

public class Example {
    public static void main(String[] args) {
        // Create client
        RatholeClient client = new RatholeClient("myserver.com", 2333, 8080);

        try {
            // Start tunnel
            client.start();

            System.out.println("Tunnel established!");
            System.out.println("Remote port: " + client.getAssignedPort());

            // Keep running
            Thread.currentThread().join();

        } catch (Exception e) {
            e.printStackTrace();
        }
    }
}
```

## Example

### 1. Start Rathole Server (Rust)

```bash
# On your VPS or server with public IP
rathole server 0.0.0.0:2333
```

### 2. Start Java Client

```bash
# On your local machine
java -jar rathole-client.jar myserver.com 2333 8080
```

Output:
```
Connecting to myserver.com:2333...
Sent TunnelRequest for local port 8080
✅ Tunnel established! Remote port: 35100
Access your service at: myserver.com:35100
Press Ctrl+C to stop...
💓 Sent heartbeat
```

### 3. Access Your Service

Now you can access your local service at `http://myserver.com:35100`

## Protocol

The protocol uses JSON messages with the following format:

```
[length: u32 little-endian][json_data: UTF-8 bytes]
```

### Message Types

#### TunnelRequest
```json
{"type":"TunnelRequest","local_port":8080}
```

#### TunnelResponse
```json
{"type":"TunnelResponse","assigned_port":35100}
```

#### CreateDataChannel
```json
{"type":"CreateDataChannel"}
```

#### Heartbeat
```json
{"type":"Heartbeat"}
```

## Architecture

```
Java Client                        Rust Server
    │                                   │
    ├─ Control Channel ────────────────┤
    │  (JSON messages)                  │
    │                                   │
    │  TunnelRequest(8080) ───────────→ │
    │ ←──────────── TunnelResponse(35100)
    │                                   │
    │ ←──────────── CreateDataChannel ──┤
    │                                   │
    ├─ Data Channel ───────────────────┤
    │  (binary, transparent)            │
    │                                   │
    │  HTTP/SSH/etc ←──────────────────→
```

## Dependencies

- [Gson](https://github.com/google/gson) - JSON library (automatically included in fat JAR)

## Compatibility

This client is compatible with:
- ✅ Rathole server v0.5.0+ (with JSON protocol)
- ✅ Any programming language that can parse JSON

## Troubleshooting

### Cannot connect to server
- Check if server is running: `telnet myserver.com 2333`
- Check firewall rules
- Verify server address and port

### Cannot connect to local service
- Ensure the local service is running on the specified port
- Try `curl http://localhost:8080` to verify

### Data channel errors
- The local service might be crashing
- Check local service logs

## License

Apache License 2.0

This client is compatible with the [original Rathole](https://github.com/rapiz1/rathole) project by rapiz1.
