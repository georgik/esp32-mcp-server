use clap::Parser;
use serde_json::Value;
use std::net::SocketAddr;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ESP32 connection error: {0}")]
    Connection(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Parser, Debug)]
#[command(name = "esp32-mcp-bridge")]
#[command(about = "Bridge between Warp and ESP32 MCP server")]
struct Args {
    /// ESP32 IP address
    #[arg(short, long, default_value = "192.168.1.100")]
    esp32_ip: String,

    /// ESP32 MCP server port
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Connection timeout in seconds
    #[arg(short, long, default_value = "10")]
    timeout: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("esp32_mcp_bridge={}", log_level))
        .with_writer(std::io::stderr)
        .init();

    info!(
        "ESP32 MCP Bridge starting - connecting to {}:{}",
        args.esp32_ip, args.port
    );

    // Create ESP32 address
    let esp32_addr: SocketAddr = format!("{}:{}", args.esp32_ip, args.port)
        .parse()
        .map_err(|e| BridgeError::Connection(format!("Invalid address: {}", e)))?;

    // Start the bridge
    run_bridge(esp32_addr, args.timeout).await?;

    Ok(())
}

async fn run_bridge(esp32_addr: SocketAddr, timeout_secs: u64) -> Result<(), BridgeError> {
    info!("Attempting to connect to ESP32 at {}", esp32_addr);

    // Connect to ESP32 MCP server with timeout
    let esp32_stream = tokio::time::timeout(
        tokio::time::Duration::from_secs(timeout_secs),
        TcpStream::connect(esp32_addr),
    )
    .await
    .map_err(|_| {
        error!("Connection timed out after {} seconds", timeout_secs);
        BridgeError::Connection(format!("Connection timeout after {} seconds", timeout_secs))
    })?
    .map_err(|e| {
        error!("TCP connection failed: {}", e);
        BridgeError::Connection(format!("Failed to connect: {}", e))
    })?;

    // Set TCP_NODELAY to reduce latency
    if let Err(e) = esp32_stream.set_nodelay(true) {
        warn!("Failed to set TCP_NODELAY: {}", e);
    }

    info!("Successfully connected to ESP32 MCP server!");

    let (esp32_reader, mut esp32_writer) = esp32_stream.into_split();
    let mut esp32_buf_reader = BufReader::new(esp32_reader).lines();

    // Set up stdin/stdout for MCP communication with Warp
    let stdin = tokio::io::stdin();
    let mut stdin_reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    info!("Bridge established - ready for MCP communication");

    loop {
        tokio::select! {
            // Read from Warp (stdin) and forward to ESP32
            line_result = stdin_reader.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() {
                            continue;
                        }

                        debug!("Received from Warp: {}", line);

                        // Validate JSON before forwarding
                        match serde_json::from_str::<Value>(&line) {
                            Ok(_) => {
                                // Forward to ESP32
                                esp32_writer.write_all(line.as_bytes()).await?;
                                esp32_writer.write_all(b"\n").await?;
                                esp32_writer.flush().await?;
                                debug!("Forwarded to ESP32: {}", line);
                            }
                            Err(e) => {
                                warn!("Invalid JSON from Warp, skipping: {}", e);

                                // Send error response back to Warp
                                let error_response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": null,
                                    "error": {
                                        "code": -32700,
                                        "message": "Parse error"
                                    }
                                });

                                let response_str = serde_json::to_string(&error_response)?;
                                stdout.write_all(response_str.as_bytes()).await?;
                                stdout.write_all(b"\n").await?;
                                stdout.flush().await?;
                            }
                        }
                    }
                    Ok(None) => {
                        info!("Warp disconnected (stdin closed)");
                        break;
                    }
                    Err(e) => {
                        error!("Error reading from Warp: {}", e);
                        break;
                    }
                }
            }

            // Read from ESP32 and forward to Warp (stdout)
            line_result = esp32_buf_reader.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() {
                            continue;
                        }

                        debug!("Received from ESP32: {}", line);

                        // Validate JSON before forwarding
                        match serde_json::from_str::<Value>(&line) {
                            Ok(_) => {
                                // Forward to Warp
                                stdout.write_all(line.as_bytes()).await?;
                                stdout.write_all(b"\n").await?;
                                stdout.flush().await?;
                                debug!("Forwarded to Warp: {}", line);
                            }
                            Err(e) => {
                                warn!("Invalid JSON from ESP32: {} - Raw: {}", e, line);
                            }
                        }
                    }
                    Ok(None) => {
                        info!("ESP32 disconnected");
                        break;
                    }
                    Err(e) => {
                        error!("Error reading from ESP32: {}", e);
                        break;
                    }
                }
            }
        }
    }

    info!("Bridge connection closed");
    Ok(())
}
