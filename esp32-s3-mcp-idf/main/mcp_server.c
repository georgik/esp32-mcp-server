#include "mcp_server.h"
#include <math.h>

static const char *TAG = "mcp_server";

// Global LED command queue
QueueHandle_t led_command_queue = NULL;

static void handle_mcp_connection(int sock);
static int handle_initialize(char *response, size_t response_size);
static int handle_tools_list(char *response, size_t response_size);
static int handle_tools_call(const cJSON *params, char *response, size_t response_size);
static int handle_wifi_status(const cJSON *arguments, char *response, size_t response_size);
static int handle_led_control(const cJSON *arguments, char *response, size_t response_size);
static int handle_compute_add(const cJSON *arguments, char *response, size_t response_size);
static int handle_compute_multiply(const cJSON *arguments, char *response, size_t response_size);

void mcp_server_task(void *pvParameters)
{
    char addr_str[128];
    int addr_family = AF_INET;
    int ip_protocol = IPPROTO_IP;
    int keepAlive = 1;
    int keepIdle = MCP_KEEPALIVE_IDLE;
    int keepInterval = MCP_KEEPALIVE_INTERVAL;
    int keepCount = MCP_KEEPALIVE_COUNT;
    struct sockaddr_storage dest_addr;

    struct sockaddr_in *dest_addr_ip4 = (struct sockaddr_in *)&dest_addr;
    dest_addr_ip4->sin_addr.s_addr = htonl(INADDR_ANY);
    dest_addr_ip4->sin_family = AF_INET;
    dest_addr_ip4->sin_port = htons(MCP_PORT);

    int listen_sock = socket(addr_family, SOCK_STREAM, ip_protocol);
    if (listen_sock < 0) {
        ESP_LOGE(TAG, "Unable to create socket: errno %d", errno);
        vTaskDelete(NULL);
        return;
    }
    
    int opt = 1;
    setsockopt(listen_sock, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
    ESP_LOGI(TAG, "Socket created");

    int err = bind(listen_sock, (struct sockaddr *)&dest_addr, sizeof(dest_addr));
    if (err != 0) {
        ESP_LOGE(TAG, "Socket unable to bind: errno %d", errno);
        goto CLEAN_UP;
    }
    ESP_LOGI(TAG, "Socket bound, port %d", MCP_PORT);

    err = listen(listen_sock, 1);
    if (err != 0) {
        ESP_LOGE(TAG, "Error occurred during listen: errno %d", errno);
        goto CLEAN_UP;
    }

    while (1) {
        ESP_LOGI(TAG, "MCP server listening on port %d", MCP_PORT);

        struct sockaddr_storage source_addr;
        socklen_t addr_len = sizeof(source_addr);
        int sock = accept(listen_sock, (struct sockaddr *)&source_addr, &addr_len);
        if (sock < 0) {
            ESP_LOGE(TAG, "Unable to accept connection: errno %d", errno);
            break;
        }

        // Set tcp keepalive option
        setsockopt(sock, SOL_SOCKET, SO_KEEPALIVE, &keepAlive, sizeof(int));
        setsockopt(sock, IPPROTO_TCP, TCP_KEEPIDLE, &keepIdle, sizeof(int));
        setsockopt(sock, IPPROTO_TCP, TCP_KEEPINTVL, &keepInterval, sizeof(int));
        setsockopt(sock, IPPROTO_TCP, TCP_KEEPCNT, &keepCount, sizeof(int));

        // Convert ip address to string
        if (source_addr.ss_family == PF_INET) {
            inet_ntoa_r(((struct sockaddr_in *)&source_addr)->sin_addr, addr_str, sizeof(addr_str) - 1);
        }
        ESP_LOGI(TAG, "MCP client connected from %s", addr_str);

        // Turn LED green to indicate client connection
        led_command_t led_cmd = {
            .type = LED_CMD_SET_COLOR,
            .r = 0, .g = 255, .b = 0,
            .brightness = 20
        };
        if (led_command_queue != NULL) {
            xQueueSend(led_command_queue, &led_cmd, 0);
        }

        handle_mcp_connection(sock);

        // Turn LED back to blue when client disconnects
        led_cmd.r = 0; led_cmd.g = 0; led_cmd.b = 255;
        if (led_command_queue != NULL) {
            xQueueSend(led_command_queue, &led_cmd, 0);
        }

        shutdown(sock, 0);
        close(sock);
        ESP_LOGI(TAG, "MCP client disconnected");
    }

CLEAN_UP:
    close(listen_sock);
    vTaskDelete(NULL);
}

static void handle_mcp_connection(int sock)
{
    char rx_buffer[MCP_BUFFER_SIZE];
    char response_buffer[MCP_BUFFER_SIZE];
    char pending_data[MCP_BUFFER_SIZE] = {0};
    int len;

    while (1) {
        len = recv(sock, rx_buffer, sizeof(rx_buffer) - 1, 0);
        if (len < 0) {
            ESP_LOGE(TAG, "Error occurred during receiving: errno %d", errno);
            break;
        } else if (len == 0) {
            ESP_LOGI(TAG, "Connection closed by client");
            break;
        }

        rx_buffer[len] = 0; // Null-terminate
        ESP_LOGI(TAG, "Received %d bytes: %s", len, rx_buffer);

        // Add received data to pending buffer
        strncat(pending_data, rx_buffer, sizeof(pending_data) - strlen(pending_data) - 1);

        // Process complete messages (separated by newlines)
        char *line_start = pending_data;
        char *newline_pos;
        
        while ((newline_pos = strchr(line_start, '\n')) != NULL) {
            *newline_pos = '\0'; // Terminate the line
            
            // Skip empty lines
            if (strlen(line_start) > 0) {
                ESP_LOGI(TAG, "Processing message: %s", line_start);
                
                int response_len = handle_mcp_request(line_start, response_buffer, sizeof(response_buffer));
                if (response_len > 0) {
                    // Add newline to response
                    if (response_len < sizeof(response_buffer) - 1) {
                        response_buffer[response_len] = '\n';
                        response_buffer[response_len + 1] = '\0';
                        response_len++;
                    }
                    
                    ESP_LOGI(TAG, "Sending response: %s", response_buffer);
                    
                    int to_write = response_len;
                    while (to_write > 0) {
                        int written = send(sock, response_buffer + (response_len - to_write), to_write, 0);
                        if (written < 0) {
                            ESP_LOGE(TAG, "Error occurred during sending: errno %d", errno);
                            return;
                        }
                        to_write -= written;
                    }
                    
                    // Small delay to ensure data is sent
                    vTaskDelay(pdMS_TO_TICKS(10));
                }
            }
            
            // Move to next line
            line_start = newline_pos + 1;
        }
        
        // Move remaining data to beginning of buffer
        memmove(pending_data, line_start, strlen(line_start) + 1);
    }
}

int handle_mcp_request(const char *request, char *response, size_t response_size)
{
    cJSON *json = cJSON_Parse(request);
    if (json == NULL) {
        ESP_LOGE(TAG, "JSON parse error");
        const char *error_response = "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32700,\"message\":\"Parse error\"}}";
        strncpy(response, error_response, response_size - 1);
        response[response_size - 1] = '\0';
        return strlen(response);
    }

    cJSON *method = cJSON_GetObjectItem(json, "method");
    cJSON *id = cJSON_GetObjectItem(json, "id");
    cJSON *params = cJSON_GetObjectItem(json, "params");

    if (method == NULL || !cJSON_IsString(method)) {
        ESP_LOGE(TAG, "Invalid method");
        cJSON_Delete(json);
        return 0;
    }

    // Handle notifications (no response needed)
    if (id == NULL || cJSON_IsNull(id)) {
        if (strcmp(method->valuestring, "notifications/initialized") == 0) {
            ESP_LOGI(TAG, "Client initialization notification received");
        }
        cJSON_Delete(json);
        return 0; // No response for notifications
    }

    char result_buffer[MCP_BUFFER_SIZE];
    int success = 0;

    if (strcmp(method->valuestring, "initialize") == 0) {
        success = handle_initialize(result_buffer, sizeof(result_buffer));
    } else if (strcmp(method->valuestring, "tools/list") == 0) {
        success = handle_tools_list(result_buffer, sizeof(result_buffer));
    } else if (strcmp(method->valuestring, "tools/call") == 0) {
        success = handle_tools_call(params, result_buffer, sizeof(result_buffer));
    } else {
        snprintf(result_buffer, sizeof(result_buffer), 
            "{\"code\":-32601,\"message\":\"Method not found\"}");
        success = -1; // Error case
    }

    // Format JSON-RPC response
    if (success > 0) {
        snprintf(response, response_size, 
            "{\"jsonrpc\":\"2.0\",\"id\":%d,\"result\":%s}",
            id->valueint, result_buffer);
    } else {
        snprintf(response, response_size,
            "{\"jsonrpc\":\"2.0\",\"id\":%d,\"error\":%s}",
            id->valueint, result_buffer);
    }

    cJSON_Delete(json);
    return strlen(response);
}

static int handle_initialize(char *response, size_t response_size)
{
    const char *init_response = "{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{\"listChanged\":false}},\"serverInfo\":{\"name\":\"esp32-s3-mcp\",\"version\":\"0.1.0\"}}";
    strncpy(response, init_response, response_size - 1);
    response[response_size - 1] = '\0';
    return strlen(response);
}

static int handle_tools_list(char *response, size_t response_size)
{
    const char *tools_response = "{\"tools\":[{\"name\":\"wifi_status\",\"description\":\"Get WiFi status\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"detailed\":{\"type\":\"boolean\"}}}},{\"name\":\"led_control\",\"description\":\"Control LED\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"color\":{\"type\":\"string\",\"enum\":[\"red\",\"green\",\"blue\",\"yellow\",\"magenta\",\"cyan\",\"white\",\"off\"]},\"r\":{\"type\":\"integer\",\"minimum\":0,\"maximum\":255},\"g\":{\"type\":\"integer\",\"minimum\":0,\"maximum\":255},\"b\":{\"type\":\"integer\",\"minimum\":0,\"maximum\":255},\"brightness\":{\"type\":\"integer\",\"minimum\":0,\"maximum\":100}}}},{\"name\":\"compute_add\",\"description\":\"Add numbers\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"number\"},\"b\":{\"type\":\"number\"}},\"required\":[\"a\",\"b\"]}},{\"name\":\"compute_multiply\",\"description\":\"Multiply numbers\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"number\"},\"b\":{\"type\":\"number\"}},\"required\":[\"a\",\"b\"]}}]}";
    strncpy(response, tools_response, response_size - 1);
    response[response_size - 1] = '\0';
    return strlen(response);
}

static int handle_tools_call(const cJSON *params, char *response, size_t response_size)
{
    if (params == NULL) {
        snprintf(response, response_size, "{\"code\":-32602,\"message\":\"Invalid params\"}");
        return -1;
    }

    cJSON *name = cJSON_GetObjectItem(params, "name");
    cJSON *arguments = cJSON_GetObjectItem(params, "arguments");

    if (name == NULL || !cJSON_IsString(name)) {
        snprintf(response, response_size, "{\"code\":-32602,\"message\":\"Missing tool name\"}");
        return -1;
    }

    if (strcmp(name->valuestring, "wifi_status") == 0) {
        return handle_wifi_status(arguments, response, response_size);
    } else if (strcmp(name->valuestring, "led_control") == 0) {
        return handle_led_control(arguments, response, response_size);
    } else if (strcmp(name->valuestring, "compute_add") == 0) {
        return handle_compute_add(arguments, response, response_size);
    } else if (strcmp(name->valuestring, "compute_multiply") == 0) {
        return handle_compute_multiply(arguments, response, response_size);
    } else {
        snprintf(response, response_size, "{\"code\":-32601,\"message\":\"Tool not found\"}");
        return -1;
    }
}

static int handle_wifi_status(const cJSON *arguments, char *response, size_t response_size)
{
    bool detailed = false;
    if (arguments != NULL) {
        cJSON *detailed_item = cJSON_GetObjectItem(arguments, "detailed");
        if (detailed_item != NULL && cJSON_IsBool(detailed_item)) {
            detailed = cJSON_IsTrue(detailed_item);
        }
    }

    if (detailed) {
        snprintf(response, response_size, 
            "{\"content\":[{\"type\":\"text\",\"text\":\"WiFi Status (Detailed):\\n- Connected: true\\n- IP: 192.168.1.100\\n- RSSI: -45 dBm\\n- SSID: MyWiFiNetwork\\n- Channel: 6\"}]}");
    } else {
        snprintf(response, response_size,
            "{\"content\":[{\"type\":\"text\",\"text\":\"WiFi Status:\\n- Connected: true\\n- IP: 192.168.1.100\"}]}");
    }
    return strlen(response);
}

static int handle_led_control(const cJSON *arguments, char *response, size_t response_size)
{
    if (arguments == NULL) {
        snprintf(response, response_size, "{\"code\":-32602,\"message\":\"LED control requires arguments\"}");
        return -1;
    }

    led_command_t led_cmd = {0};
    uint8_t r = 255, g = 255, b = 255;
    uint8_t brightness = 20;
    bool color_set = false;

    // Check for predefined colors
    cJSON *color = cJSON_GetObjectItem(arguments, "color");
    if (color != NULL && cJSON_IsString(color)) {
        if (strcmp(color->valuestring, "red") == 0) {
            r = 255; g = 0; b = 0; color_set = true;
        } else if (strcmp(color->valuestring, "green") == 0) {
            r = 0; g = 255; b = 0; color_set = true;
        } else if (strcmp(color->valuestring, "blue") == 0) {
            r = 0; g = 0; b = 255; color_set = true;
        } else if (strcmp(color->valuestring, "yellow") == 0) {
            r = 255; g = 255; b = 0; color_set = true;
        } else if (strcmp(color->valuestring, "magenta") == 0) {
            r = 255; g = 0; b = 255; color_set = true;
        } else if (strcmp(color->valuestring, "cyan") == 0) {
            r = 0; g = 255; b = 255; color_set = true;
        } else if (strcmp(color->valuestring, "white") == 0) {
            r = 255; g = 255; b = 255; color_set = true;
        } else if (strcmp(color->valuestring, "off") == 0) {
            led_cmd.type = LED_CMD_OFF;
            if (led_command_queue != NULL) {
                xQueueSend(led_command_queue, &led_cmd, portMAX_DELAY);
            }
            snprintf(response, response_size, "{\"content\":[{\"type\":\"text\",\"text\":\"LED turned off\"}]}");
            return strlen(response);
        }
    }

    // Parse individual RGB components if not using predefined color
    if (!color_set) {
        cJSON *r_item = cJSON_GetObjectItem(arguments, "r");
        if (r_item != NULL && cJSON_IsNumber(r_item)) {
            r = (uint8_t)cJSON_GetNumberValue(r_item);
        }

        cJSON *g_item = cJSON_GetObjectItem(arguments, "g");
        if (g_item != NULL && cJSON_IsNumber(g_item)) {
            g = (uint8_t)cJSON_GetNumberValue(g_item);
        }

        cJSON *b_item = cJSON_GetObjectItem(arguments, "b");
        if (b_item != NULL && cJSON_IsNumber(b_item)) {
            b = (uint8_t)cJSON_GetNumberValue(b_item);
        }
    }

    // Parse brightness
    cJSON *brightness_item = cJSON_GetObjectItem(arguments, "brightness");
    if (brightness_item != NULL && cJSON_IsNumber(brightness_item)) {
        brightness = (uint8_t)fmin(100, fmax(0, cJSON_GetNumberValue(brightness_item)));
    }

    // Send LED command
    led_cmd.type = LED_CMD_SET_COLOR;
    led_cmd.r = r;
    led_cmd.g = g;
    led_cmd.b = b;
    led_cmd.brightness = brightness;

    if (led_command_queue != NULL) {
        xQueueSend(led_command_queue, &led_cmd, portMAX_DELAY);
    }

    snprintf(response, response_size,
        "{\"content\":[{\"type\":\"text\",\"text\":\"LED set to RGB(%d, %d, %d) with %d%% brightness\"}]}",
        r, g, b, brightness);
    return strlen(response);
}

static int handle_compute_add(const cJSON *arguments, char *response, size_t response_size)
{
    if (arguments == NULL) {
        snprintf(response, response_size, "{\"code\":-32602,\"message\":\"Addition requires arguments\"}");
        return -1;
    }

    cJSON *a_item = cJSON_GetObjectItem(arguments, "a");
    cJSON *b_item = cJSON_GetObjectItem(arguments, "b");

    if (a_item == NULL || !cJSON_IsNumber(a_item) || b_item == NULL || !cJSON_IsNumber(b_item)) {
        snprintf(response, response_size, "{\"code\":-32602,\"message\":\"Both 'a' and 'b' parameters required\"}");
        return -1;
    }

    double a = cJSON_GetNumberValue(a_item);
    double b = cJSON_GetNumberValue(b_item);
    double result = a + b;

    snprintf(response, response_size,
        "{\"content\":[{\"type\":\"text\",\"text\":\"%.2f + %.2f = %.2f\"}]}",
        a, b, result);
    return strlen(response);
}

static int handle_compute_multiply(const cJSON *arguments, char *response, size_t response_size)
{
    if (arguments == NULL) {
        snprintf(response, response_size, "{\"code\":-32602,\"message\":\"Multiplication requires arguments\"}");
        return -1;
    }

    cJSON *a_item = cJSON_GetObjectItem(arguments, "a");
    cJSON *b_item = cJSON_GetObjectItem(arguments, "b");

    if (a_item == NULL || !cJSON_IsNumber(a_item) || b_item == NULL || !cJSON_IsNumber(b_item)) {
        snprintf(response, response_size, "{\"code\":-32602,\"message\":\"Both 'a' and 'b' parameters required\"}");
        return -1;
    }

    double a = cJSON_GetNumberValue(a_item);
    double b = cJSON_GetNumberValue(b_item);
    double result = a * b;

    snprintf(response, response_size,
        "{\"content\":[{\"type\":\"text\",\"text\":\"%.2f × %.2f = %.2f\"}]}",
        a, b, result);
    return strlen(response);
}
