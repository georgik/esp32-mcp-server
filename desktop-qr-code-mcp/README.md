# QR Code MCP Server

A simple Model Context Protocol (MCP) server that generates QR codes from text and displays them in the terminal.

## Features

- Generates QR codes from any text input
- Displays QR codes directly in the terminal
- Compatible with MCP protocol version 2024-11-05
- Built with Rust for performance and reliability

## Installation

```bash
# Build the project
cargo build --release

# Run the server
cargo run --release
```

## Usage

This is an MCP server that communicates via JSON-RPC over stdin/stdout. It's designed to be used by AI agents or MCP clients.

### Available Tools

- `generate_qr_code`: Generate a QR code from text and display it in the terminal
  - Parameters:
    - `text` (required): The text to encode in the QR code

### Example MCP Requests

#### Initialize the server
```json
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
```

#### List available tools
```json
{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}
```

#### Generate a QR code
```json
{
  "jsonrpc": "2.0", 
  "id": 3, 
  "method": "tools/call", 
  "params": {
    "name": "generate_qr_code",
    "arguments": {
      "text": "Hello, World!"
    }
  }
}
```

## How it works

1. The server listens for JSON-RPC requests on stdin
2. When a `generate_qr_code` tool is called, it:
   - Creates a QR code from the provided text
   - Displays the QR code in the terminal using ASCII characters
   - Returns the QR code as text in the response
3. The server sends responses back via stdout

## Dependencies

- `qrcode`: For QR code generation
- `tokio`: For async runtime
- `serde`: For JSON serialization/deserialization
- `tracing`: For logging
- `thiserror`: For error handling

## Testing

You can test the server manually by running it and sending JSON-RPC requests:

```bash
# In one terminal
cargo run --release

# In another terminal, send a request
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}' | cargo run --release
```

Or create a simple test script to interact with the server.

## License

This project is open source and available under the MIT License.
