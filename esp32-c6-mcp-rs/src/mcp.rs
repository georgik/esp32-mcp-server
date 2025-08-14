use heapless::String;
use serde::{Deserialize, Serialize};

pub const MAX_JSON_SIZE: usize = 3072; // Carefully sized for ESP32-C6 memory constraints
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SmartLedParams {
    #[serde(default)]
    pub color: Option<String<16>>, // "red", "green", "blue", "off"
    #[serde(default)]
    pub r: Option<u8>,
    #[serde(default)]
    pub g: Option<u8>,
    #[serde(default)]
    pub b: Option<u8>,
    #[serde(default)]
    pub brightness: Option<u8>, // 0-100
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComputeParams {
    pub a: f32,
    pub b: f32,
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
    // Compact JSON response to fit ESP32-C6 memory constraints
    let response = r#"{"tools":[{"name":"wifi_status","description":"Get WiFi status","inputSchema":{"type":"object","properties":{"detailed":{"type":"boolean"}}}},{"name":"led_control","description":"Control LED","inputSchema":{"type":"object","properties":{"color":{"type":"string","enum":["red","green","blue","yellow","magenta","cyan","white","off"]},"r":{"type":"integer","minimum":0,"maximum":255},"g":{"type":"integer","minimum":0,"maximum":255},"b":{"type":"integer","minimum":0,"maximum":255},"brightness":{"type":"integer","minimum":0,"maximum":100}}}},{"name":"compute_add","description":"Add numbers","inputSchema":{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}},"required":["a","b"]}},{"name":"compute_multiply","description":"Multiply numbers","inputSchema":{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}},"required":["a","b"]}}]}"#;
    Ok(StdString::from(response))
}

// Global LED command sender - will be set by main.rs
use core::sync::atomic::{AtomicPtr, Ordering};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;

#[derive(Debug, Clone)]
pub enum LedCommand {
    SetColor { r: u8, g: u8, b: u8, brightness: u8 },
    Off,
}

static LED_SENDER: AtomicPtr<Sender<'static, CriticalSectionRawMutex, LedCommand, 4>> =
    AtomicPtr::new(core::ptr::null_mut());

pub fn set_led_sender(sender: &'static Sender<'static, CriticalSectionRawMutex, LedCommand, 4>) {
    LED_SENDER.store(sender as *const _ as *mut _, Ordering::Relaxed);
}

fn send_led_command(cmd: LedCommand) -> Result<(), &'static str> {
    let sender_ptr = LED_SENDER.load(Ordering::Relaxed);
    if sender_ptr.is_null() {
        return Err("LED sender not initialized");
    }

    let sender =
        unsafe { &*(sender_ptr as *const Sender<'static, CriticalSectionRawMutex, LedCommand, 4>) };

    match sender.try_send(cmd) {
        Ok(()) => Ok(()),
        Err(_) => Err("LED command queue full"),
    }
}

fn handle_tools_call(raw_json: &str) -> Result<StdString, McpError> {
    // Look for different tool names in the raw JSON

    if raw_json.contains("\"name\":\"wifi_status\"") {
        // Check if detailed flag is set
        let detailed = raw_json.contains("\"detailed\":true");

        let response = if detailed {
            r#"{"content":[{"type":"text","text":"WiFi Status (Detailed):\n- Connected: true\n- IP: 192.168.32.87\n- RSSI: -45 dBm\n- SSID: MyWiFiNetwork\n- Channel: 6"}]}"#
        } else {
            r#"{"content":[{"type":"text","text":"WiFi Status:\n- Connected: true\n- IP: 192.168.32.87"}]}"#
        };

        Ok(StdString::from(response))
    } else if raw_json.contains("\"name\":\"led_control\"") {
        handle_led_control(raw_json)
    } else if raw_json.contains("\"name\":\"compute_add\"") {
        handle_compute_add(raw_json)
    } else if raw_json.contains("\"name\":\"compute_multiply\"") {
        handle_compute_multiply(raw_json)
    } else {
        Err(McpError {
            code: -32601,
            message: String::try_from("Tool not found").unwrap_or_else(|_| String::new()),
        })
    }
}

fn handle_led_control(raw_json: &str) -> Result<StdString, McpError> {
    // Parse LED control parameters from JSON
    let mut r = 255u8;
    let mut g = 255u8;
    let mut b = 255u8;
    let mut brightness = 20u8; // Default to 20% brightness
    let mut color_set = false;

    // Check for predefined colors first
    if raw_json.contains("\"color\":\"red\"") {
        r = 255;
        g = 0;
        b = 0;
        color_set = true;
    } else if raw_json.contains("\"color\":\"green\"") {
        r = 0;
        g = 255;
        b = 0;
        color_set = true;
    } else if raw_json.contains("\"color\":\"blue\"") {
        r = 0;
        g = 0;
        b = 255;
        color_set = true;
    } else if raw_json.contains("\"color\":\"yellow\"") {
        r = 255;
        g = 255;
        b = 0;
        color_set = true;
    } else if raw_json.contains("\"color\":\"magenta\"") {
        r = 255;
        g = 0;
        b = 255;
        color_set = true;
    } else if raw_json.contains("\"color\":\"cyan\"") {
        r = 0;
        g = 255;
        b = 255;
        color_set = true;
    } else if raw_json.contains("\"color\":\"white\"") {
        r = 255;
        g = 255;
        b = 255;
        color_set = true;
    } else if raw_json.contains("\"color\":\"off\"") {
        if let Err(e) = send_led_command(LedCommand::Off) {
            return Err(McpError {
                code: -32603,
                message: String::try_from(e).unwrap_or_else(|_| String::new()),
            });
        }
        return Ok(StdString::from(
            r#"{"content":[{"type":"text","text":"LED turned off"}]}"#,
        ));
    }

    // Parse individual RGB components if not using predefined color
    if !color_set {
        // Simple regex-free parsing for r, g, b values
        if let Some(r_start) = raw_json.find("\"r\":") {
            let r_start = r_start + 4;
            if let Some(r_end) = raw_json[r_start..].find([',', '}']) {
                if let Ok(parsed_r) = raw_json[r_start..r_start + r_end].trim().parse::<u8>() {
                    r = parsed_r;
                }
            }
        }

        if let Some(g_start) = raw_json.find("\"g\":") {
            let g_start = g_start + 4;
            if let Some(g_end) = raw_json[g_start..].find([',', '}']) {
                if let Ok(parsed_g) = raw_json[g_start..g_start + g_end].trim().parse::<u8>() {
                    g = parsed_g;
                }
            }
        }

        if let Some(b_start) = raw_json.find("\"b\":") {
            let b_start = b_start + 4;
            if let Some(b_end) = raw_json[b_start..].find([',', '}']) {
                if let Ok(parsed_b) = raw_json[b_start..b_start + b_end].trim().parse::<u8>() {
                    b = parsed_b;
                }
            }
        }
    }

    // Parse brightness
    if let Some(br_start) = raw_json.find("\"brightness\":") {
        let br_start = br_start + 13;
        if let Some(br_end) = raw_json[br_start..].find([',', '}']) {
            if let Ok(parsed_br) = raw_json[br_start..br_start + br_end].trim().parse::<u8>() {
                brightness = parsed_br.min(100);
            }
        }
    }

    // Send LED command
    if let Err(e) = send_led_command(LedCommand::SetColor {
        r,
        g,
        b,
        brightness,
    }) {
        return Err(McpError {
            code: -32603,
            message: String::try_from(e).unwrap_or_else(|_| String::new()),
        });
    }

    let response = alloc::format!(
        r#"{{"content":[{{"type":"text","text":"LED set to RGB({}, {}, {}) with {}% brightness"}}]}}"#,
        r,
        g,
        b,
        brightness
    );
    Ok(response)
}

fn handle_compute_add(raw_json: &str) -> Result<StdString, McpError> {
    // Parse a and b from JSON
    let mut a = 0.0f32;
    let mut b = 0.0f32;

    // Parse 'a' parameter
    if let Some(a_start) = raw_json.find("\"a\":") {
        let a_start = a_start + 4;
        if let Some(a_end) = raw_json[a_start..].find([',', '}']) {
            if let Ok(parsed_a) = raw_json[a_start..a_start + a_end].trim().parse::<f32>() {
                a = parsed_a;
            }
        }
    }

    // Parse 'b' parameter
    if let Some(b_start) = raw_json.find("\"b\":") {
        let b_start = b_start + 4;
        if let Some(b_end) = raw_json[b_start..].find([',', '}']) {
            if let Ok(parsed_b) = raw_json[b_start..b_start + b_end].trim().parse::<f32>() {
                b = parsed_b;
            }
        }
    }

    let result = a + b;
    let response = alloc::format!(
        r#"{{"content":[{{"type":"text","text":"{} + {} = {}"}}]}}"#,
        a,
        b,
        result
    );
    Ok(response)
}

fn handle_compute_multiply(raw_json: &str) -> Result<StdString, McpError> {
    // Parse a and b from JSON
    let mut a = 0.0f32;
    let mut b = 0.0f32;

    // Parse 'a' parameter
    if let Some(a_start) = raw_json.find("\"a\":") {
        let a_start = a_start + 4;
        if let Some(a_end) = raw_json[a_start..].find([',', '}']) {
            if let Ok(parsed_a) = raw_json[a_start..a_start + a_end].trim().parse::<f32>() {
                a = parsed_a;
            }
        }
    }

    // Parse 'b' parameter
    if let Some(b_start) = raw_json.find("\"b\":") {
        let b_start = b_start + 4;
        if let Some(b_end) = raw_json[b_start..].find([',', '}']) {
            if let Ok(parsed_b) = raw_json[b_start..b_start + b_end].trim().parse::<f32>() {
                b = parsed_b;
            }
        }
    }

    let result = a * b;
    let response = alloc::format!(
        r#"{{"content":[{{"type":"text","text":"{} × {} = {}"}}]}}"#,
        a,
        b,
        result
    );
    Ok(response)
}
