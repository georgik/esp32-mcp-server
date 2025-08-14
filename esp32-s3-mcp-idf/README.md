# ESP32-S3 MCP Server (ESP-IDF)

This is an ESP-IDF C implementation of the MCP (Model Context Protocol) server for ESP32-S3, providing the same functionality as the Rust version but built with the ESP-IDF framework.

## Features

- **MCP Protocol Support**: Full JSON-RPC 2.0 implementation with MCP methods
- **WiFi Connectivity**: Connects to WiFi network and serves MCP over TCP on port 3000
- **Addressable LED Control**: WS2812-compatible LED strip control via GPIO8 (configurable)
- **Multiple Tools**: WiFi status, LED control, and computation tools
- **Visual Status**: LED color indicates system status (blue=ready, green=client connected)

## Hardware Requirements

- **ESP32-S3 Development Board**
- **Addressable LED Strip**: WS2812/NeoPixel compatible, connected to GPIO8
- **WiFi Network**: 2.4GHz network for connectivity

## Wiring

| Component | ESP32-S3 Pin | Notes |
|-----------|--------------|-------|
| LED Strip Data | GPIO8 | Configurable in menuconfig |
| LED Strip VCC | 3.3V or 5V | Depends on LED strip |
| LED Strip GND | GND | Common ground |

## Build and Flash

### Prerequisites

1. Install ESP-IDF v5.0 or later:
   ```bash
   git clone --recursive https://github.com/espressif/esp-idf.git
   cd esp-idf
   ./install.sh
   . ./export.sh
   ```

2. Set WiFi credentials:
   ```bash
   cd esp32-s3-mcp-idf
   idf.py menuconfig
   ```
   Navigate to `Example Connection Configuration` and set your WiFi SSID and password.

### Configuration

In menuconfig, you can also configure:
- **ESP32-S3 MCP Server Configuration**:
  - MCP Server Port (default: 3000)
  - TCP keep-alive settings
  - LED GPIO pin (default: 8)

### Build and Flash

```bash
# Clean build
idf.py fullclean

# Build
idf.py build

# Flash to ESP32-S3
idf.py -p /dev/ttyUSB0 flash

# Monitor serial output
idf.py -p /dev/ttyUSB0 monitor
```

## Usage

### LED Status Indicators

| Color | Status |
|-------|--------|
| Blue (20% brightness) | System ready, no MCP client |
| Green (20% brightness) | MCP client connected |

### Available MCP Tools

1. **wifi_status** - Get WiFi connection information
2. **led_control** - Control the LED strip
3. **compute_add** - Add two numbers
4. **compute_multiply** - Multiply two numbers

### LED Control Examples

Connect via the bridge tool or directly via TCP:

```bash
# Turn LED red
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"led_control","arguments":{"color":"red","brightness":50}}}' | nc [ESP_IP] 3000

# Set custom RGB color
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"led_control","arguments":{"r":255,"g":128,"b":0,"brightness":30}}}' | nc [ESP_IP] 3000

# Turn LED off
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"led_control","arguments":{"color":"off"}}}' | nc [ESP_IP] 3000
```

## Technical Details

### Memory Usage

- **RAM Usage**: ~200KB for WiFi, TCP stack, and MCP processing
- **Flash Usage**: ~1.5MB including ESP-IDF framework
- **Stack Sizes**: 
  - MCP server task: 8KB
  - LED control task: 4KB

### Performance

- **TCP Connections**: 1 concurrent client
- **JSON Processing**: Up to 4KB messages using cJSON library
- **LED Updates**: Real-time via FreeRTOS queue system
- **Response Time**: <10ms for simple commands

### Architecture

```
┌─────────────────┐    ┌──────────────────┐
│   MCP Client    │◄──►│  TCP Server      │
│  (Warp/Bridge)  │    │  (Port 3000)     │
└─────────────────┘    └──────────────────┘
                                │
                                ▼
                       ┌──────────────────┐
                       │  JSON-RPC        │
                       │  Request Handler │
                       └──────────────────┘
                                │
                  ┌─────────────┼─────────────┐
                  ▼             ▼             ▼
            ┌──────────┐ ┌──────────┐ ┌──────────┐
            │WiFi Tools│ │LED Tools │ │Math Tools│
            └──────────┘ └──────────┘ └──────────┘
                                │
                                ▼
                       ┌──────────────────┐
                       │  FreeRTOS Queue  │
                       └──────────────────┘
                                │
                                ▼
                       ┌──────────────────┐
                       │  LED Controller  │
                       │  (RMT + WS2812)  │
                       └──────────────────┘
```

### Configuration Files

- `Kconfig.projbuild` - Project-specific configuration options
- `CMakeLists.txt` - Build configuration
- `sdkconfig` - Generated ESP-IDF configuration (created after menuconfig)

## Troubleshooting

### Common Issues

1. **LED not working**:
   - Check wiring connections
   - Verify GPIO pin in menuconfig matches hardware
   - Ensure LED strip is WS2812 compatible

2. **WiFi connection fails**:
   - Double-check SSID and password in menuconfig
   - Ensure 2.4GHz network (ESP32-S3 doesn't support 5GHz)

3. **MCP client can't connect**:
   - Check ESP32-S3 IP address in serial monitor
   - Verify port 3000 is not blocked by firewall
   - Ensure both devices are on same network

### Debug Logs

Enable debug logs for specific components:
```bash
idf.py menuconfig
# Component config -> Log output -> Default log verbosity -> Debug
```

Monitor with timestamps:
```bash
idf.py monitor --print-filter "mcp_server:D" --print-filter "led_control:D"
```

## Differences from Rust Version

| Aspect | ESP-IDF (C) | Rust |
|--------|-------------|------|
| Framework | ESP-IDF | Embassy async |
| Language | C | Rust |
| Memory Safety | Manual | Automatic |
| JSON Parsing | cJSON | serde-json-core |
| Task Model | FreeRTOS | Embassy tasks |
| Error Handling | Error codes | Result types |

## Development

### Adding New Tools

1. Add handler function in `mcp_server.c`
2. Update `handle_tools_call()` to route new tool
3. Update `handle_tools_list()` to include new tool schema

### Customizing LED Behavior

Edit `led_control.c` to:
- Change default colors
- Add animation effects
- Support multiple LEDs
- Implement different LED protocols

## Related Projects

- [ESP32-C6 MCP Server (Rust)](../esp32-c6-mcp-rs/) - Rust implementation for ESP32-C6
- [MCP Bridge Tool](../esp32-mcp-bridge/) - Warp terminal integration
