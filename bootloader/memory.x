/* SPDX-License-Identifier: GPL-3.0-only */

MEMORY
{
  FLASH      : ORIGIN = 0x08000000, LENGTH = 0x7FA00
  TRAMPOLINE : ORIGIN = 0x0807FC00, LENGTH = 0x400
  RAM        : ORIGIN = 0x24000000, LENGTH = 256K
}

/* Leave one aligned KiB for the STM32H725 vector table. */
_stext = ORIGIN(FLASH) + 0x400;

SECTIONS
{
  .epsilon_keypad_rows 0x0800AC60 :
  {
    KEEP(*(.epsilon_keypad_rows));
  } > FLASH

  .epsilon_keypad_columns 0x0800B356 :
  {
    KEEP(*(.epsilon_keypad_columns));
  } > FLASH

  .trampoline ORIGIN(TRAMPOLINE) :
  {
    KEEP(*(.trampoline));
  } > TRAMPOLINE
}
INSERT AFTER .rodata;

ASSERT(SIZEOF(.trampoline) == 13 * 4, "invalid Epsilon trampoline table");
ASSERT(SIZEOF(.epsilon_keypad_rows) == 9, "invalid Epsilon keypad rows");
ASSERT(SIZEOF(.epsilon_keypad_columns) == 12, "invalid Epsilon keypad columns");
