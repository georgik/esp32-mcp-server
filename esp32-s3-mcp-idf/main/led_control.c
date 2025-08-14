#include "mcp_server.h"
#include "driver/rmt_tx.h"
#include "led_strip_encoder.h"
#include <math.h>

static const char *TAG = "led_control";

// LED configuration for ESP32-S3
#define RMT_LED_STRIP_RESOLUTION_HZ 10000000 // 10MHz resolution, 1 tick = 0.1us

#ifdef CONFIG_LED_GPIO
#define RMT_LED_STRIP_GPIO_NUM CONFIG_LED_GPIO
#else
#define RMT_LED_STRIP_GPIO_NUM 8        // GPIO8 for LED strip (similar to C6)
#endif

#define LED_NUMBERS                 1        // Single addressable LED
#define LED_STRIP_TASK_STACK_SIZE   4096
#define LED_STRIP_TASK_PRIORITY     5

static uint8_t led_strip_pixels[LED_NUMBERS * 3];
static rmt_channel_handle_t led_chan = NULL;
static rmt_encoder_handle_t led_encoder = NULL;

// Gamma correction table for better color representation
static const uint8_t gamma8[] = {
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  1,  1,  1,  1,
    1,  1,  1,  1,  1,  1,  1,  1,  1,  2,  2,  2,  2,  2,  2,  2,
    2,  3,  3,  3,  3,  3,  3,  3,  4,  4,  4,  4,  4,  5,  5,  5,
    5,  6,  6,  6,  6,  7,  7,  7,  7,  8,  8,  8,  9,  9,  9, 10,
   10, 10, 11, 11, 11, 12, 12, 13, 13, 13, 14, 14, 15, 15, 16, 16,
   17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 24, 24, 25,
   25, 26, 27, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 35, 36,
   37, 38, 39, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 50,
   51, 52, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 66, 67, 68,
   69, 70, 72, 73, 74, 75, 77, 78, 79, 81, 82, 83, 85, 86, 87, 89,
   90, 92, 93, 95, 96, 98, 99,101,102,104,105,107,109,110,112,114,
  115,117,119,120,122,124,126,127,129,131,133,135,137,138,140,142,
  144,146,148,150,152,154,156,158,160,162,164,167,169,171,173,175,
  177,180,182,184,186,189,191,193,196,198,200,203,205,208,210,213,
  215,218,220,223,225,228,231,233,236,239,241,244,247,249,252,255
};

// Apply gamma correction and brightness scaling
static uint8_t apply_gamma_brightness(uint8_t color, uint8_t brightness_percent)
{
    // Scale brightness from percentage (0-100) to 0-255
    uint16_t brightness = (brightness_percent * 255) / 100;
    
    // Apply brightness scaling first, then gamma correction
    uint16_t scaled = (color * brightness) / 255;
    return gamma8[scaled];
}

esp_err_t led_init(void)
{
    ESP_LOGI(TAG, "Initializing LED strip on GPIO%d", RMT_LED_STRIP_GPIO_NUM);

    // Create RMT TX channel
    rmt_tx_channel_config_t tx_chan_config = {
        .clk_src = RMT_CLK_SRC_DEFAULT,
        .gpio_num = RMT_LED_STRIP_GPIO_NUM,
        .mem_block_symbols = 64,
        .resolution_hz = RMT_LED_STRIP_RESOLUTION_HZ,
        .trans_queue_depth = 4,
    };
    ESP_ERROR_CHECK(rmt_new_tx_channel(&tx_chan_config, &led_chan));

    // Install LED strip encoder
    led_strip_encoder_config_t encoder_config = {
        .resolution = RMT_LED_STRIP_RESOLUTION_HZ,
    };
    ESP_ERROR_CHECK(rmt_new_led_strip_encoder(&encoder_config, &led_encoder));

    // Enable RMT TX channel
    ESP_ERROR_CHECK(rmt_enable(led_chan));

    // Initialize LED to blue (system ready)
    esp_err_t ret = led_set_color(0, 0, 255, 20);
    if (ret == ESP_OK) {
        ESP_LOGI(TAG, "LED initialized to blue (system ready)");
    }

    return ret;
}

esp_err_t led_set_color(uint8_t r, uint8_t g, uint8_t b, uint8_t brightness)
{
    if (led_chan == NULL || led_encoder == NULL) {
        ESP_LOGE(TAG, "LED not initialized");
        return ESP_ERR_INVALID_STATE;
    }

    // Apply gamma correction and brightness
    uint8_t corrected_r = apply_gamma_brightness(r, brightness);
    uint8_t corrected_g = apply_gamma_brightness(g, brightness);
    uint8_t corrected_b = apply_gamma_brightness(b, brightness);

    // WS2812 LED format: GRB order
    led_strip_pixels[0] = corrected_g;
    led_strip_pixels[1] = corrected_r;
    led_strip_pixels[2] = corrected_b;

    // Transmit to LED strip
    rmt_transmit_config_t tx_config = {
        .loop_count = 0, // no transfer loop
    };

    esp_err_t ret = rmt_transmit(led_chan, led_encoder, led_strip_pixels, sizeof(led_strip_pixels), &tx_config);
    if (ret == ESP_OK) {
        ret = rmt_tx_wait_all_done(led_chan, pdMS_TO_TICKS(100));
        if (ret == ESP_OK) {
            ESP_LOGI(TAG, "LED set to RGB(%d, %d, %d) with %d%% brightness", r, g, b, brightness);
        } else {
            ESP_LOGE(TAG, "LED transmit wait failed: %s", esp_err_to_name(ret));
        }
    } else {
        ESP_LOGE(TAG, "LED transmit failed: %s", esp_err_to_name(ret));
    }

    return ret;
}

esp_err_t led_turn_off(void)
{
    ESP_LOGI(TAG, "Turning LED off");
    return led_set_color(0, 0, 0, 0);
}

void led_control_task(void *pvParameters)
{
    ESP_LOGI(TAG, "LED control task started");
    
    led_command_t led_cmd;
    
    while (1) {
        if (xQueueReceive(led_command_queue, &led_cmd, portMAX_DELAY) == pdTRUE) {
            switch (led_cmd.type) {
                case LED_CMD_SET_COLOR:
                    led_set_color(led_cmd.r, led_cmd.g, led_cmd.b, led_cmd.brightness);
                    break;
                
                case LED_CMD_OFF:
                    led_turn_off();
                    break;
                
                default:
                    ESP_LOGW(TAG, "Unknown LED command type: %d", led_cmd.type);
                    break;
            }
            
            // Small delay to prevent overwhelming the LED controller
            vTaskDelay(pdMS_TO_TICKS(10));
        }
    }
}
