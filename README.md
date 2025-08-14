# ESP32-C6 MCP Server

This project implements an MCP (Model Context Protocol) server running on ESP32-C6 that connects to WiFi, gets an IP address via DHCP, and exposes MCP functionality that can be accessed by Warp.

## Project Structure

- `esp32-c6-mcp-rs/` - ESP32-C6 firmware with WiFi and MCP server
- `esp32-mcp-bridge/` - Rust bridge tool for connecting Warp to ESP32 MCP server
- `desktop-qr-code-mcp/` - Reference desktop MCP server for QR code generation

## Features

- **WiFi Connection**: Automatically connects to WiFi and obtains IP via DHCP
- **MCP Server**: Implements MCP protocol over TCP on port 3000
- **WiFi Status Tool**: MCP tool to check WiFi connection status and network info
- **Bridge Tool**: Pure Rust bridge for connecting Warp to ESP32 MCP server

## Prerequisites

- Rust toolchain with ESP32 target support
- ESP-HAL v1.0.0-rc.0
- WiFi network credentials (SSID and PASSWORD)

## Building and Flashing ESP32-C6 Firmware

1. Set environment variables for WiFi credentials:
   ```bash
   export SSID="YourWiFiNetwork"
   export PASSWORD="YourWiFiPassword"
   ```

2. Build and flash the firmware:
   ```bash
   cd esp32-c6-mcp-rs
   cargo build --release  # Always build with --release for embedded targets
   cargo run --release    # This will flash the firmware
   ```

3. Monitor the output to see the IP address assigned by DHCP:
   ```bash
   cargo monitor
   ```

## Building the Bridge Tool

The bridge tool allows Warp to communicate with the ESP32 MCP server:

```bash
cd esp32-mcp-bridge
cargo build --release
```

## Using the Bridge with Warp

1. First, make sure your ESP32-C6 is running and connected to WiFi
2. Note the IP address from the ESP32 serial output
3. Configure Warp to use the bridge tool as an MCP server

### Option 1: Direct Command Line Usage

```bash
./target/release/esp32-mcp-bridge --esp32-ip 192.168.1.100 --port 3000
```

### Option 2: Warp Configuration

Add this to your Warp MCP configuration:

```json
{
  "mcpServers": {
    "esp32-mcp": {
      "command": "/path/to/esp32-mcp-server/esp32-mcp-bridge/target/release/esp32-mcp-bridge",
      "args": ["--esp32-ip", "192.168.1.100", "--port", "3000"]
    }
  }
}
```

## Available MCP Tools

The ESP32 MCP server currently provides:

### `wifi_status`
- **Description**: Get WiFi connection status and IP information
- **Parameters**: 
  - `detailed` (optional boolean): Include detailed connection info
- **Returns**: Current WiFi status, IP address, signal strength, and connected SSID

## Architecture

```
Warp Terminal
    ↓ (stdin/stdout JSON-RPC)
ESP32 MCP Bridge (Rust)
    ↓ (TCP JSON-RPC)
ESP32-C6 MCP Server
    ↓ (WiFi)
Router/Network
```

The bridge tool handles the protocol translation between Warp's stdin/stdout MCP communication and the ESP32's TCP-based MCP server.

## Development

### ESP32-C6 Development

- The firmware uses Embassy async runtime for efficient task handling
- WiFi credentials are set via environment variables at compile time
- JSON processing uses `serde-json-core` for no_std compatibility
- Heap allocation is used for network buffers (128KB heap)

### Bridge Development  

- Built with Tokio for async TCP networking
- Handles bidirectional JSON-RPC message forwarding
- Includes connection timeout and error handling
- Supports verbose logging for debugging

## Troubleshooting

### ESP32 Not Connecting to WiFi
- Check SSID and PASSWORD environment variables
- Verify WiFi network is 2.4GHz (ESP32-C6 doesn't support 5GHz)
- Check serial output for connection errors

### Bridge Connection Issues
- Ensure ESP32 has obtained an IP address via DHCP
- Verify the IP address in bridge command matches ESP32's IP
- Check firewall settings on your network
- Try with `--verbose` flag for detailed logging

### Warp Integration Issues
- Verify the bridge tool path in Warp configuration
- Check that the bridge tool builds successfully
- Ensure ESP32 is running and accessible before starting Warp

## Network Security Notes

- This is a development/prototype implementation
- WiFi credentials are embedded in firmware at compile time
- The MCP server accepts connections from any client on the network
- For production use, consider adding authentication and encryption
