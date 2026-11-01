/* SPDX-License-Identifier: GPL-3.0-only */

MEMORY
{
  FLASH      : ORIGIN = 0x08000000, LENGTH = 0x7FA00
  TRAMPOLINE : ORIGIN = 0x0807FC00, LENGTH = 0x400
  /* Keep the bootloader separate from Epsilon's AXI SRAM. */
  RAM        : ORIGIN = 0x20000000, LENGTH = 128K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);

/* Stock bootloader header at 0x08000244. Without it Epsilon 26.4+ overwrites internal flash with
   its own bootloader and sets RDP1. These vector slots are unused, so they can hold the header. */
BDMA_CH0 = 0xDEC0B007;
BDMA_CH1 = 1;
BDMA_CH2 = 0xDEC0B007;
_stext = ALIGN(ADDR(.boot_services) + SIZEOF(.boot_services), 4);

SECTIONS
{
  .boot_services ORIGIN(FLASH) + 0x400 :
  {
    *(.text.*epsilon_compat*)
    *(.text.*n0120_hardware3spi*)
    *(.text.*n0120_hardware3mcu7suspend*)
  } > FLASH

  .ram_vectors (NOLOAD) :
  {
    __ram_vectors = .;
    . += 0x400;
  } > RAM
}
INSERT BEFORE .text;

/* Let cortex-m-rt copy the code along with .data. */
SECTIONS
{
  .ram_code : AT(LOADADDR(.data) + ADDR(.ram_code) - ADDR(.data))
  {
    *(.text .text.*);
    *(.HardFaultTrampoline);
    *(.HardFault.*);
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > RAM
}
INSERT AFTER .data;

ASSERT(ADDR(.ram_vectors) % 0x400 == 0, "unaligned RAM vector table");
ASSERT(LOADADDR(.ram_code) + SIZEOF(.ram_code) <= ORIGIN(FLASH) + LENGTH(FLASH),
       "bootloader overlaps factory metadata");

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
}

INSERT AFTER .text;

SECTIONS
{
  .trampoline ORIGIN(TRAMPOLINE) :
  {
    KEEP(*(.trampoline));
  } > TRAMPOLINE
}
INSERT AFTER .rodata;

ASSERT(SIZEOF(.trampoline) == 13 * 4, "invalid Epsilon trampoline table");
ASSERT(SIZEOF(.epsilon_keypad_rows) == 9, "invalid Epsilon keypad rows");
ASSERT(SIZEOF(.epsilon_keypad_columns) == 12, "invalid Epsilon keypad columns");
