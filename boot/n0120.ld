/* SPDX-License-Identifier: GPL-3.0-only */

ENTRY(reset_handler)

MEMORY
{
  FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 0x80000
  SRAM  (rwx) : ORIGIN = 0x24000000, LENGTH = 0x40000
}

_stack_top = ORIGIN(SRAM) + LENGTH(SRAM);

SECTIONS
{
  .vectors ORIGIN(FLASH) : ALIGN(128)
  {
    KEEP(*(.vectors))
  } > FLASH

  .text :
  {
    *(.text .text.*)
    *(.rodata .rodata.*)
  } > FLASH

  /DISCARD/ :
  {
    *(.ARM.exidx*)
    *(.ARM.extab*)
    *(.comment)
  }
}
