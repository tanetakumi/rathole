# Rathole Protocol Specification

## Overview

Rathole uses a simple JSON-based protocol for control messages. This makes it easy to implement clients in any programming language.

**Protocol Version**: 1.0
**Encoding**: JSON (UTF-8)
**Message Format**: `[length: u32 little-endian][json_data: UTF-8 bytes]`

## Message Format

Each message consists of:
1. **Length prefix**: 4 bytes (u32, little-endian) indicating the length of the JSON data
2. **JSON data**: UTF-8 encoded JSON object

```
┌─────────────────┬──────────────────────────┐
│  Length (u32)   │      JSON Data           │
│  (little-endian)│      (UTF-8)             │
└─────────────────┴──────────────────────────┘
     4 bytes            variable length
```

## Connection Types

### 1. Control Channel
- Persistent connection for control messages
- Handles tunnel management and heartbeats
- One control channel per client

### 2. Data Channel
- Created on-demand for data forwarding
- Transparent binary data transfer (no JSON)
- Multiple data channels per tunnel

## Message Types

All messages have a `type` field that identifies the message type.

### TunnelRequest

**Direction**: Client → Server
**Purpose**: Request to create a new tunnel

```json
{
  "type": "TunnelRequest",
  "local_port": 8080
}
```

**Fields**:
- `type`: "TunnelRequest"
- `local_port`: The local port number to forward (u16)

### TunnelResponse

**Direction**: Server → Client
**Purpose**: Confirm tunnel creation and provide assigned port

```json
{
  "type": "TunnelResponse",
  "assigned_port": 35100
}
```

**Fields**:
- `type`: "TunnelResponse"
- `assigned_port`: The remote port assigned by the server (u16)

### CreateDataChannel

**Direction**: Server → Client
**Purpose**: Request client to create a new data channel

```json
{
  "type": "CreateDataChannel"
}
```

**Fields**:
- `type`: "CreateDataChannel"

### Heartbeat

**Direction**: Bidirectional
**Purpose**: Keep-alive and connection health check

```json
{
  "type": "Heartbeat"
}
```

**Fields**:
- `type`: "Heartbeat"

## Protocol Flow

### Initial Handshake

```
Client                          Server
  │                               │
  │─── Connect TCP ───────────────│
  │                               │
  │─── TunnelRequest ────────────→│
  │    {local_port: 8080}         │
  │                               │ (Allocate port 35100)
  │                               │ (Start listener on 35100)
  │                               │
  │←── TunnelResponse ────────────│
  │    {assigned_port: 35100}     │
  │                               │
```

### Data Channel Creation

```
Visitor connects to server:35100
         │
         ↓
Server ─── CreateDataChannel ───→ Client
         │                         │
         │                         │ (Create new connection)
         │←─── TCP Connect ────────│
         │                         │
         │                         │ (Connect to localhost:8080)
         │                         │
    [Start bidirectional copy]
         │                         │
    Visitor ←──────────────────→ Local Service
```

### Heartbeat

```
Client                          Server
  │                               │
  │─────── Heartbeat ────────────→│
  │         (every 20s)            │
  │                               │
  │←────── Heartbeat ─────────────│
  │         (every 20s)            │
  │                               │
```

**Timeouts**:
- Client: 60 seconds without any message → reconnect
- Server: 60 seconds without any message → close connection

## Implementation Guidelines

### Sending Messages

1. Serialize message to JSON string
2. Encode JSON string as UTF-8 bytes
3. Write length as u32 little-endian
4. Write JSON data
5. Flush output stream

**Example (pseudo-code)**:
```
json_string = json_encode(message)
data = utf8_encode(json_string)
length = len(data)

write_u32_le(length)
write_bytes(data)
flush()
```

### Receiving Messages

1. Read 4 bytes as u32 little-endian (length)
2. Read `length` bytes (JSON data)
3. Decode as UTF-8 string
4. Parse JSON
5. Validate message type

**Example (pseudo-code)**:
```
length = read_u32_le()
data = read_bytes(length)
json_string = utf8_decode(data)
message = json_parse(json_string)

switch message.type:
    case "TunnelResponse": ...
    case "CreateDataChannel": ...
    case "Heartbeat": ...
```

### Data Channel

Data channels are **pure TCP streams** - no JSON encoding!

```
[Client Local Service] ←─→ [Client] ←─→ [Server] ←─→ [Visitor]
        HTTP/SSH/etc.          Binary data (transparent)
```

The server and client just copy bytes bidirectionally without any processing.

## Error Handling

### Connection Errors
- If control channel disconnects, client should reconnect automatically
- Retry interval: 3 seconds
- Exponential backoff is recommended

### Timeout Handling
- If no message received for 60 seconds, assume connection is dead
- Client: reconnect to server
- Server: close connection and clean up resources

### Invalid Messages
- Unknown message type → log warning, ignore message
- Invalid JSON → close connection
- Message too large (>1MB) → close connection

## Security Notes

⚠️ **This simplified version has NO authentication**

- All messages are sent in plaintext
- No encryption
- No token-based authentication
- Only use in trusted networks

For production use, consider:
- TLS for encryption
- Token-based authentication
- Rate limiting
- IP whitelisting

## Implementation Examples

### Rust (Server/Client)
See `src/protocol.rs` for the reference implementation.

### Java (Client)
See `clients/java/src/main/java/com/rathole/RatholeClient.java` for a complete Java implementation.

### Python (Client Example)

```python
import socket
import json
import struct

def send_message(sock, msg):
    json_data = json.dumps(msg).encode('utf-8')
    length = struct.pack('<I', len(json_data))
    sock.sendall(length + json_data)

def receive_message(sock):
    length_data = sock.recv(4)
    length = struct.unpack('<I', length_data)[0]
    json_data = sock.recv(length).decode('utf-8')
    return json.loads(json_data)

# Connect
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.connect(('server.com', 2333))

# Send TunnelRequest
send_message(sock, {'type': 'TunnelRequest', 'local_port': 8080})

# Receive TunnelResponse
response = receive_message(sock)
print(f"Assigned port: {response['assigned_port']}")
```

## Version History

### Version 1.0 (Current)
- Initial JSON-based protocol
- Support for 4 message types
- Little-endian length encoding
- UTF-8 JSON encoding

## License

Apache License 2.0

This protocol documentation is part of the rathole project.
