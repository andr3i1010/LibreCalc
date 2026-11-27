/* SPDX-License-Identifier: GPL-3.0-only */

MEMORY
{
  HEADER   : ORIGIN = 0x90000000, LENGTH = 0x400
  /* Ends where the shared UI library starts (tools/library.ld). */
  FLASH    : ORIGIN = 0x90000400, LENGTH = 0x7FC00
  RAM      : ORIGIN = 0x20000000, LENGTH = 56K
  STACK    : ORIGIN = 0x2000E000, LENGTH = 8K
}

_stack_start = ORIGIN(STACK) + LENGTH(STACK);
_stack_end = ORIGIN(STACK);

ASSERT(ORIGIN(RAM) + LENGTH(RAM) <= ORIGIN(STACK), "kernel RAM overlaps kernel stack");
ASSERT(ORIGIN(STACK) + LENGTH(STACK) <= 0x20010000, "kernel outgrew its 64 KiB of DTCM");

PROVIDE(__app_header = 0);

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
