#ifndef MCP_SERVER_H
#define MCP_SERVER_H

#include <stdio.h>
#include <string.h>
#include <sys/param.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/queue.h"
#include "esp_system.h"
#include "esp_log.h"
#include "lwip/err.h"
#include "lwip/sockets.h"
#include "lwip/sys.h"
#include <lwip/netdb.h>
#include "cJSON.h"

// MCP Server Configuration
#ifdef CONFIG_MCP_PORT
#define MCP_PORT CONFIG_MCP_PORT
#else
#define MCP_PORT 3000
#endif

#define MCP_BUFFER_SIZE 4096
#define MCP_MAX_CLIENTS 1

#ifdef CONFIG_MCP_KEEPALIVE_IDLE
#define MCP_KEEPALIVE_IDLE CONFIG_MCP_KEEPALIVE_IDLE
#else
#define MCP_KEEPALIVE_IDLE 7
#endif

#ifdef CONFIG_MCP_KEEPALIVE_INTERVAL
#define MCP_KEEPALIVE_INTERVAL CONFIG_MCP_KEEPALIVE_INTERVAL
#else
#define MCP_KEEPALIVE_INTERVAL 1
#endif

#ifdef CONFIG_MCP_KEEPALIVE_COUNT
#define MCP_KEEPALIVE_COUNT CONFIG_MCP_KEEPALIVE_COUNT
#else
#define MCP_KEEPALIVE_COUNT 3
#endif

// LED Control Commands
typedef enum {
    LED_CMD_SET_COLOR,
    LED_CMD_OFF
} led_command_type_t;

typedef struct {
    led_command_type_t type;
    uint8_t r, g, b;
    uint8_t brightness;
} led_command_t;

// Function declarations
void mcp_server_task(void *pvParameters);
int handle_mcp_request(const char *request, char *response, size_t response_size);
void led_control_task(void *pvParameters);
esp_err_t led_init(void);
esp_err_t led_set_color(uint8_t r, uint8_t g, uint8_t b, uint8_t brightness);
esp_err_t led_turn_off(void);

// Global LED command queue
extern QueueHandle_t led_command_queue;

#endif // MCP_SERVER_H
