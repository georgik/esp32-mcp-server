use heapless::String;
use serde::{Deserialize, Serialize};

pub const MAX_JSON_SIZE: usize = 2048;
pub const MAX_PARAMS_SIZE: usize = 512;
pub const MAX_RESULT_SIZE: usize = 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String<16>,
    pub id: Option<u32>,
    pub method: String<32>,
    #[serde(default, skip_deserializing)]
    pub params: Option<()>,
}

extern crate alloc;
use alloc::string::String as StdString;

// Simple response struct - we'll handle JSON serialization manually
#[derive(Debug)]
pub struct McpResponse {
    pub jsonrpc: String<16>,
    pub id: Option<u32>,
    pub result: Option<StdString>,
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String<128>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WifiStatusParams {
    #[serde(default)]
    pub detailed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String<32>,
    #[serde(default)]
    pub arguments: Option<WifiStatusParams>,
}

#[derive(Debug, Serialize)]
pub struct WifiStatusResult {
    pub connected: bool,
    pub ip_address: Option<String<16>>,
    pub rssi: Option<i8>,
    pub ssid: Option<String<32>>,
}

pub fn handle_mcp_request(request: &McpRequest, raw_json: &str) -> McpResponse {
    let result = match request.method.as_str() {
        "initialize" => handle_initialize(),
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tools_call(raw_json),
        _ => Err(McpError {
            code: -32601,
            message: String::try_from("Method not found").unwrap_or_else(|_| String::new()),
        }),
    };

    match result {
        Ok(res) => McpResponse {
            jsonrpc: String::try_from("2.0").unwrap_or_else(|_| String::new()),
            id: request.id,
            result: Some(res),
            error: None,
        },
        Err(err) => McpResponse {
            jsonrpc: String::try_from("2.0").unwrap_or_else(|_| String::new()),
            id: request.id,
            result: None,
            error: Some(err),
        },
    }
}

fn handle_initialize() -> Result<StdString, McpError> {
    let response = r#"{"protocolVersion":"2024-11-05","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"esp32-c6-mcp","version":"0.1.0"}}"#;
    Ok(StdString::from(response))
}

fn handle_tools_list() -> Result<StdString, McpError> {
    let response = r#"{"tools":[{"name":"wifi_status","description":"Get WiFi connection status and IP information","inputSchema":{"type":"object","properties":{"detailed":{"type":"boolean","description":"Include detailed connection info"}}}}]}"#;
    Ok(StdString::from(response))
}


fn handle_tools_call(raw_json: &str) -> Result<StdString, McpError> {
    // Manually extract the tool name and arguments from the raw JSON
    // This is a simple approach that works with our specific use case
    
    // Look for "name":"wifi_status"
    if raw_json.contains("\"name\":\"wifi_status\"") {
        // Check if detailed flag is set
        let detailed = raw_json.contains("\"detailed\":true");
        
        let response = if detailed {
            r#"{"content":[{"type":"text","text":"WiFi Status (Detailed):\n- Connected: true\n- IP: 192.168.32.87\n- RSSI: -45 dBm\n- SSID: MyWiFiNetwork\n- Channel: 6"}]}"#
        } else {
            r#"{"content":[{"type":"text","text":"WiFi Status:\n- Connected: true\n- IP: 192.168.32.87"}]}"#
        };
        
        Ok(StdString::from(response))
    } else {
        Err(McpError {
            code: -32601,
            message: String::try_from("Tool not found").unwrap_or_else(|_| String::new()),
        })
    }
}
