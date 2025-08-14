# Warp AI Terminal Setup for QR MCP Server

This guide explains how to configure Warp AI Terminal to use the QR Code MCP Server.

## Configuration Steps

### 1. Build the Server
Make sure the server is built first:
```bash
cargo build --release
```

### 2. Create Warp Configuration Directory
```bash
mkdir -p ~/.config/warp
```

### 3. Create MCP Servers Configuration
Create or edit the file `~/.config/warp/mcp_servers.json`:

```json
{
  "mcpServers": {
    "qr-generator": {
      "command": ".../target/release/qr-mcp-server",
      "args": [],
      "env": {
        "RUST_LOG": "qr_mcp_server=info"
      }
    }
  }
}
```

### 4. Restart Warp
After creating the configuration file, restart Warp AI Terminal for the changes to take effect.

## Usage in Warp AI

Once configured, you can use the QR generator in Warp AI by asking questions like:

- "Generate a QR code for 'Hello, World!'"
- "Create a QR code for this URL: https://github.com"
- "Make a QR code for my WiFi password"

The AI will use the `generate_qr_code` tool to create QR codes and display them directly in your terminal.

## Configuration Options

### Alternative Configuration Using Cargo
If you prefer to use `cargo run` instead of the binary:

```json
{
  "mcpServers": {
    "qr-generator": {
      "command": "cargo",
      "args": ["run", "--release"],
      "cwd": "...",
      "env": {}
    }
  }
}
```

### Environment Variables
- `RUST_LOG`: Controls logging level (debug, info, warn, error)
- You can add other environment variables as needed

## Troubleshooting

1. **Server not found**: Make sure the binary path is correct and the server was built successfully
2. **Permission issues**: Ensure the binary is executable: `chmod +x target/release/qr-mcp-server`
3. **Configuration not loaded**: Restart Warp after making configuration changes
4. **Path issues**: Use absolute paths in the configuration to avoid issues

## Verifying the Setup

You can test the server manually:
```bash
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "generate_qr_code", "arguments": {"text": "Test"}}}' | ./target/release/qr-mcp-server
```

This should display a QR code in your terminal and return a JSON response.
