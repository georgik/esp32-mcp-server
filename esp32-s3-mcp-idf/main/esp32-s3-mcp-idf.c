#include "mcp_server.h"
#include "esp_wifi.h"
#include "esp_event.h"
#include "nvs_flash.h"
#include "esp_netif.h"
#include "protocol_examples_common.h"

static const char *TAG = "main";

void app_main(void)
{
    // Initialize NVS
    esp_err_t ret = nvs_flash_init();
    if (ret == ESP_ERR_NVS_NO_FREE_PAGES || ret == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_ERROR_CHECK(nvs_flash_erase());
        ret = nvs_flash_init();
    }
    ESP_ERROR_CHECK(ret);

    // Initialize networking
    ESP_ERROR_CHECK(esp_netif_init());
    ESP_ERROR_CHECK(esp_event_loop_create_default());

    // This helper function configures Wi-Fi or Ethernet, as selected in menuconfig
    ESP_ERROR_CHECK(example_connect());

    // Initialize LED
    ESP_ERROR_CHECK(led_init());
    ESP_LOGI(TAG, "LED initialized successfully");

    // Create LED command queue
    led_command_queue = xQueueCreate(8, sizeof(led_command_t));
    if (led_command_queue == NULL) {
        ESP_LOGE(TAG, "Failed to create LED command queue");
        return;
    }

    // Create LED control task
    BaseType_t led_task_created = xTaskCreate(
        led_control_task,
        "led_control",
        4096,
        NULL,
        5,
        NULL
    );
    
    if (led_task_created != pdPASS) {
        ESP_LOGE(TAG, "Failed to create LED control task");
        return;
    }
    
    ESP_LOGI(TAG, "LED control task created successfully");

    // Create MCP server task
    BaseType_t mcp_task_created = xTaskCreate(
        mcp_server_task,
        "mcp_server",
        8192,  // Larger stack for JSON processing
        NULL,
        5,
        NULL
    );
    
    if (mcp_task_created != pdPASS) {
        ESP_LOGE(TAG, "Failed to create MCP server task");
        return;
    }
    
    ESP_LOGI(TAG, "ESP32-S3 MCP Server started successfully!");
    ESP_LOGI(TAG, "- WiFi connected");
    ESP_LOGI(TAG, "- LED control ready on GPIO8");
    ESP_LOGI(TAG, "- MCP server listening on port %d", MCP_PORT);
    ESP_LOGI(TAG, "System ready - LED should be blue");
}
