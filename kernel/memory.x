/* SPDX-License-Identifier: GPL-3.0-only */

MEMORY
{
  HEADER   : ORIGIN = 0x90000000, LENGTH = 0x400
  FLASH    : ORIGIN = 0x90000400, LENGTH = 0x3EFC00
  RAM      : ORIGIN = 0x24000000, LENGTH = 56K
  STACK    : ORIGIN = 0x2400E000, LENGTH = 8K
  USER_RAM : ORIGIN = 0x24010000, LENGTH = 192K
}

_stack_start = ORIGIN(STACK) + LENGTH(STACK);
_stack_end = ORIGIN(STACK);

ASSERT(ORIGIN(RAM) + LENGTH(RAM) <= ORIGIN(STACK), "kernel RAM overlaps kernel stack");
ASSERT(ORIGIN(STACK) + LENGTH(STACK) <= ORIGIN(USER_RAM), "kernel stack overlaps userspace RAM");
_user_ram_start = ORIGIN(USER_RAM);
_user_ram_end = ORIGIN(USER_RAM) + LENGTH(USER_RAM);

/* Leave one aligned KiB for the STM32H725 vector table. */
_stext = ORIGIN(FLASH) + 0x400;
_image_end = LOADADDR(.data) + SIZEOF(.data);

SECTIONS
{
  .image_header ORIGIN(HEADER) :
  {
    LONG(1)
    LONG(_image_end - ORIGIN(HEADER))
    LONG(0x4C41434C)
    KEEP(*(.image_header.version))
    LONG(0)
    LONG(0)
    LONG(0x4C41434C)
    LONG(_stack_start);
    LONG(Reset);
    . = ORIGIN(HEADER) + LENGTH(HEADER);
  } > HEADER
}
INSERT BEFORE .vector_table;
