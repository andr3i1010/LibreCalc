/* SPDX-License-Identifier: GPL-3.0-only */

#include <stdint.h>

extern uintptr_t _stack_top;

void reset_handler(void);

static volatile uint16_t *const lcd_command = (volatile uint16_t *)0x60000000u;
static volatile uint16_t *const lcd_data = (volatile uint16_t *)0x60020000u;

static const uint8_t hello[][5] = {
    {0x7f, 0x08, 0x08, 0x08, 0x7f},
    {0x38, 0x54, 0x54, 0x54, 0x18},
    {0x00, 0x41, 0x7f, 0x40, 0x00},
    {0x00, 0x41, 0x7f, 0x40, 0x00},
    {0x38, 0x44, 0x44, 0x44, 0x38},
    {0x00, 0x50, 0x30, 0x00, 0x00},
    {0x00, 0x00, 0x00, 0x00, 0x00},
    {0x7c, 0x40, 0x30, 0x40, 0x7c},
    {0x38, 0x44, 0x44, 0x44, 0x38},
    {0x7c, 0x08, 0x04, 0x04, 0x08},
    {0x00, 0x41, 0x7f, 0x40, 0x00},
    {0x38, 0x44, 0x44, 0x48, 0x7f},
    {0x00, 0x00, 0x5f, 0x00, 0x00},
};

__attribute__((section(".vectors"), used))
const uintptr_t vector_table[] = {
    (uintptr_t)&_stack_top,
    (uintptr_t)reset_handler,
};

static void lcd_window(uint16_t x0, uint16_t y0, uint16_t x1, uint16_t y1) {
  *lcd_command = 0x2a;
  *lcd_data = x0 >> 8;
  *lcd_data = x0;
  *lcd_data = x1 >> 8;
  *lcd_data = x1;

  *lcd_command = 0x2b;
  *lcd_data = y0 >> 8;
  *lcd_data = y0;
  *lcd_data = y1 >> 8;
  *lcd_data = y1;

  *lcd_command = 0x2c;
}

void reset_handler(void) {
  *lcd_command = 0x11;
  *lcd_command = 0x36;
  *lcd_data = 0xa0;
  *lcd_command = 0x29;

  lcd_window(0, 0, 319, 239);
  for (uint32_t i = 0; i < 320u * 240u; i++) {
    *lcd_data = 0x0000;
  }

  lcd_window(43, 109, 276, 129);
  for (uint32_t y = 0; y < 21; y++) {
    for (uint32_t c = 0; c < 13; c++) {
      for (uint32_t x = 0; x < 18; x++) {
        uint32_t column = x / 3;
        *lcd_data = column < 5 && (hello[c][column] & (1u << (y / 3)))
                        ? 0xffff
                        : 0x0000;
      }
    }
  }

  for (;;) {
    __asm volatile("wfi");
  }
}
