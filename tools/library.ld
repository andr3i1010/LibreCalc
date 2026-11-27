/* SPDX-License-Identifier: GPL-3.0-only */

/* Links the shared UI library at its fixed place in flash (graphite_ui's TABLE). It runs in the
   calling app's context, so it can't have mutable statics of its own. */
ENTRY(GRAPHITE_UI_TABLE)

SECTIONS
{
  .table 0x90080000 : { KEEP(*(.graphite_ui_table)) }
  .text : { *(.text .text.*) }
  .rodata : { *(.rodata .rodata.*) }
  .ARM.exidx : { *(.ARM.exidx .ARM.exidx.*) }
  .data : { *(.data .data.*) }
  .bss : { *(.bss .bss.* COMMON) }
}

ASSERT(SIZEOF(.data) + SIZEOF(.bss) == 0, "a shared library can't have mutable statics");
ASSERT(ADDR(.bss) <= 0x90100000, "the UI library outgrew its 512 KiB");
