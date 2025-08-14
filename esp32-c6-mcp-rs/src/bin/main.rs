#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_net::{Runner, StackResources, tcp::TcpSocket, Stack};
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_wifi::{
    EspWifiController,
    init,
    wifi::{ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiState},
};
use log::{info, warn, error};
use esp32_c6_mcp_rs::mcp::{McpRequest, handle_mcp_request, MAX_JSON_SIZE};
use embedded_io_async::{Read, Write};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

// WiFi credentials - in production, these should come from secure storage
const SSID: &str = env!("SSID", "WiFi SSID must be set via environment variable");
const PASSWORD: &str = env!("PASSWORD", "WiFi password must be set via environment variable");
const MCP_PORT: u16 = 3000;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Increased heap size for networking and JSON processing
    esp_alloc::heap_allocator!(size: 128 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let mut rng = Rng::new(peripherals.RNG);

    let esp_wifi_ctrl = &*mk_static!(
        EspWifiController<'static>,
        init(timg0.timer0, rng.clone()).unwrap()
    );

    let (controller, interfaces) = esp_wifi::wifi::new(&esp_wifi_ctrl, peripherals.WIFI).unwrap();
    let wifi_interface = interfaces.sta;

    let systimer = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(systimer.alarm0);

    let config = embassy_net::Config::dhcpv4(Default::default());
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // Initialize network stack
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );
    
    let stack = mk_static!(Stack<'static>, stack);

    spawner.spawn(connection_task(controller)).ok();
    spawner.spawn(net_task(runner)).ok();
    spawner.spawn(mcp_server_task(stack)).ok();

    info!("ESP32-C6 MCP Server starting...");
    info!("Connecting to WiFi: {}", SSID);

    // Wait for network link
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    info!("WiFi link is up!");

    // Wait for IP address from DHCP
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP address: {}", config.address);
            info!("MCP Server listening on port {}", MCP_PORT);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    // Main loop
    loop {
        Timer::after(Duration::from_secs(10)).await;
        info!("MCP Server running...");
    }
}

#[embassy_executor::task]
async fn connection_task(mut controller: WifiController<'static>) {
    info!("WiFi connection task started");
    info!("Device capabilities: {:?}", controller.capabilities());
    
    // https://docs.esp-rs.org/esp-hal/esp-wifi/0.12.0/esp32c6/esp_wifi/#wifi-performance-considerations
    info!("Disabling PowerSaveMode to avoid delay when receiving data.");
    controller.set_power_saving(esp_wifi::config::PowerSaveMode::None).unwrap();
    
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                // Wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.into(),
                password: PASSWORD.into(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            info!("Starting WiFi...");
            controller.start_async().await.unwrap();
            info!("WiFi started!");
        }
        
        info!("Attempting to connect to WiFi...");
        match controller.connect_async().await {
            Ok(_) => info!("Successfully connected to WiFi!"),
            Err(e) => {
                error!("Failed to connect to WiFi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn mcp_server_task(stack: &'static Stack<'static>) {
    info!("MCP server task starting...");
    
    loop {
        // Wait until we have an IP address
        if stack.config_v4().is_none() {
            Timer::after(Duration::from_millis(100)).await;
            continue;
        }

        // Create fresh buffers for each connection
        let mut rx_buffer = [0; 4096];
        let mut tx_buffer = [0; 4096];
        let mut socket = TcpSocket::new(*stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(30)));

        info!("MCP server listening on port {}...", MCP_PORT);
        
        match socket.accept(MCP_PORT).await {
            Ok(()) => {
                info!("MCP client connected!");
                
                // Handle the connection
                match handle_mcp_connection(&mut socket).await {
                    Ok(()) => info!("MCP client disconnected normally"),
                    Err(e) => error!("MCP connection error: {:?}", e),
                }
            }
            Err(e) => {
                error!("Socket accept error: {:?}", e);
                Timer::after(Duration::from_millis(1000)).await;
            }
        }
    }
}

async fn handle_mcp_connection<T: Read + Write>(
    socket: &mut T,
) -> Result<(), T::Error>
where
    T::Error: core::fmt::Debug,
{
    let mut buffer = [0u8; MAX_JSON_SIZE];
    let mut response_buf = [0u8; MAX_JSON_SIZE];
    let mut pending_data = String::new();
    
    loop {
        info!("Waiting for MCP request...");
        
        // Read new data from socket
        match socket.read(&mut buffer).await {
            Ok(0) => {
                info!("MCP connection closed by client (no data received)");
                return Ok(());
            }
            Ok(n) => {
                info!("Received {} bytes of data", n);
                
                // Convert to string and add to pending data
                match core::str::from_utf8(&buffer[..n]) {
                    Ok(new_data) => {
                        pending_data.push_str(new_data);
                    }
                    Err(_) => {
                        warn!("Invalid UTF-8 in received data");
                        continue;
                    }
                }
                
                // Process all complete messages (separated by newlines)
                while let Some(newline_pos) = pending_data.find('\n') {
                    let message = pending_data[..newline_pos].trim().to_string();
                    pending_data = pending_data[newline_pos + 1..].to_string();
                    
                    if message.is_empty() {
                        continue;
                    }
                    
                    info!("Processing message ({}bytes): {}", message.len(), message);
                    
                    // Process this complete message
                    if let Err(e) = process_mcp_message(socket, &message, &mut response_buf).await {
                        error!("Error processing message: {:?}", e);
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                error!("Socket read error: {:?}", e);
                return Err(e);
            }
        }
    }
}

async fn process_mcp_message<T: Write>(
    socket: &mut T,
    request_str: &str,
    response_buf: &mut [u8; MAX_JSON_SIZE],
) -> Result<(), T::Error>
where
    T::Error: core::fmt::Debug,
{
    info!("Attempting to parse JSON...");
    
    // Parse and handle MCP request
    match serde_json_core::from_str::<McpRequest>(request_str) {
            Ok((request, _)) => {
                info!("Successfully parsed MCP request: method={}", request.method.as_str());
                
                // Check if this is a notification (no id field)
                if request.id.is_none() {
                    info!("Processing notification: {}", request.method.as_str());
                    
                    // For notifications, just handle them but don't send a response
                    match request.method.as_str() {
                        "notifications/initialized" => {
                            info!("Client initialization notification received - connection ready");
                        }
                        _ => {
                            warn!("Unknown notification method: {}", request.method.as_str());
                        }
                    }
                    
                    // Return without sending a response for notifications
                    return Ok(());
                }
                
                let response = handle_mcp_request(&request, request_str);
                
                // Manually construct JSON to avoid double-encoding the result field
                let response_str = if let Some(ref result) = response.result {
                    // Construct JSON with raw result (not escaped)
                    let id_str = match response.id {
                        Some(id) => id.to_string(),
                        None => "null".to_string(),
                    };
                    let mut response_json = String::new();
                    response_json.push_str("{\"jsonrpc\":\"");
                    response_json.push_str(response.jsonrpc.as_str());
                    response_json.push_str("\",\"id\":");
                    response_json.push_str(&id_str);
                    response_json.push_str(",\"result\":");
                    response_json.push_str(result);
                    response_json.push_str("}");
                    response_json
                } else if let Some(ref error) = response.error {
                    // Serialize error normally
                    match serde_json_core::to_string::<_, MAX_JSON_SIZE>(&error) {
                        Ok(error_json) => {
                            let id_str = match response.id {
                                Some(id) => id.to_string(),
                                None => "null".to_string(),
                            };
                            let mut response_json = String::new();
                            response_json.push_str("{\"jsonrpc\":\"");
                            response_json.push_str(response.jsonrpc.as_str());
                            response_json.push_str("\",\"id\":");
                            response_json.push_str(&id_str);
                            response_json.push_str(",\"error\":");
                            response_json.push_str(&error_json);
                            response_json.push_str("}");
                            response_json
                        },
                        Err(e) => {
                            error!("Failed to serialize error: {:?}", e);
                            return Ok(());
                        }
                    }
                } else {
                    return Ok(()); // Invalid response
                };
                
                info!("Successfully constructed response ({}bytes)", response_str.len());
                
                info!("Sending MCP response: {}", response_str);
                
                // Send response
                response_buf[..response_str.len()].copy_from_slice(response_str.as_bytes());
                response_buf[response_str.len()] = b'\n';
                
                if let Err(e) = socket.write_all(&response_buf[..response_str.len() + 1]).await {
                    error!("Write error: {:?}", e);
                    return Err(e);
                }
                
                // CRITICAL: Flush the socket to ensure data is actually sent
                if let Err(e) = socket.flush().await {
                    error!("Flush error: {:?}", e);
                    return Err(e);
                }
                
                // Give client time to receive the response before potentially closing connection
                Timer::after(Duration::from_millis(10)).await;
                
                info!("Response sent and flushed successfully");
            }
            Err(e) => {
                error!("JSON parse failed: {:?}", e);
                error!("Raw request bytes: {:?}", request_str.as_bytes());
                
                // Send error response
                let error_response = r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}\n"#;
                info!("Sending error response: {}", error_response);
                
                if let Err(e) = socket.write_all(error_response.as_bytes()).await {
                    error!("Write error: {:?}", e);
                    return Err(e);
                }
                
                // CRITICAL: Flush the socket to ensure error response is actually sent
                if let Err(e) = socket.flush().await {
                    error!("Error response flush error: {:?}", e);
                    return Err(e);
                }
                
                // Give client time to receive the error response
                Timer::after(Duration::from_millis(10)).await;
                
                info!("Error response sent and flushed successfully");
            }
        }
    Ok(())
}
