use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader};
use tracing::{debug, error, info};

#[derive(Error, Debug, Serialize, Deserialize)]
pub enum McpError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("JSON error: {0}")]
    Json(String),
    #[error("QR code generation error: {0}")]
    QrCode(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

impl From<io::Error> for McpError {
    fn from(err: io::Error) -> Self {
        McpError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for McpError {
    fn from(err: serde_json::Error) -> Self {
        McpError::Json(err.to_string())
    }
}

impl From<qrcode::types::QrError> for McpError {
    fn from(err: qrcode::types::QrError) -> Self {
        McpError::QrCode(err.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct McpRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct QrCodeParams {
    text: String,
    #[serde(default = "default_size")]
    size: Option<String>,
}

fn default_size() -> Option<String> {
    Some("medium".to_string())
}

#[derive(Debug, Serialize)]
struct QrCodeResult {
    qr_code: String,
    text: String,
}

fn generate_qr_code(text: &str) -> Result<String, McpError> {
    let code = QrCode::new(text)?;
    let string = code
        .render::<char>()
        .quiet_zone(false)
        .module_dimensions(2, 1)
        .build();
    Ok(string)
}

fn handle_qr_request(params: QrCodeParams) -> Result<QrCodeResult, McpError> {
    info!("Generating QR code for text: {}", params.text);
    let qr_code = generate_qr_code(&params.text)?;

    // Print QR code to terminal
    println!("\n🔲 QR Code for: {}\n", params.text);
    println!("{}", qr_code);
    println!();

    Ok(QrCodeResult {
        qr_code,
        text: params.text,
    })
}

fn handle_initialize_request() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "qr-mcp-server",
            "version": "0.1.0"
        }
    })
}

fn handle_tools_list_request() -> serde_json::Value {
    serde_json::json!({
        "tools": [
            {
                "name": "generate_qr_code",
                "description": "Generate a QR code from text and display it in the terminal",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The text to encode in the QR code"
                        }
                    },
                    "required": ["text"]
                }
            }
        ]
    })
}

fn handle_tools_call_request(params: serde_json::Value) -> Result<serde_json::Value, McpError> {
    let tool_params: serde_json::Value = params
        .get("arguments")
        .ok_or_else(|| McpError::InvalidRequest("Missing arguments".to_string()))?
        .clone();

    let qr_params: QrCodeParams = serde_json::from_value(tool_params)
        .map_err(|e| McpError::InvalidRequest(format!("Invalid QR code parameters: {}", e)))?;

    let result = handle_qr_request(qr_params)?;

    Ok(serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": format!("QR code generated successfully for: {}\n\nQR Code (also displayed in terminal):\n{}", result.text, result.qr_code)
            }
        ]
    }))
}

fn process_request(request: McpRequest) -> McpResponse {
    let result = match request.method.as_str() {
        "initialize" => Ok(handle_initialize_request()),
        "tools/list" => Ok(handle_tools_list_request()),
        "tools/call" => match request.params {
            Some(params) => handle_tools_call_request(params),
            None => Err(McpError::InvalidRequest(
                "Missing parameters for tools/call".to_string(),
            )),
        },
        _ => Err(McpError::InvalidRequest(format!(
            "Unknown method: {}",
            request.method
        ))),
    };

    match result {
        Ok(res) => McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(res),
            error: None,
        },
        Err(err) => {
            error!("Request failed: {}", err);
            McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(err),
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("qr_mcp_server=debug")
        .with_writer(std::io::stderr)
        .init();

    info!("QR Code MCP Server starting...");

    let stdin = tokio::io::stdin();
    let mut reader = AsyncBufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        debug!("Received request: {}", line);

        match serde_json::from_str::<McpRequest>(&line) {
            Ok(request) => {
                let response = process_request(request);
                let response_json = serde_json::to_string(&response)?;
                debug!("Sending response: {}", response_json);
                stdout.write_all(response_json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
            Err(e) => {
                error!("Failed to parse request: {}", e);
                let error_response = McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(McpError::Json(e.to_string())),
                };
                let response_json = serde_json::to_string(&error_response)?;
                stdout.write_all(response_json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }
    }

    info!("QR Code MCP Server shutting down...");
    Ok(())
}
