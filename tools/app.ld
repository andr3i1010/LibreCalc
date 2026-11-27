/* SPDX-License-Identifier: GPL-3.0-only */

/* Links a native app (.lca) into place: pass --defsym=__app_flash=<address in app flash>. */
ENTRY(_start)

/* App globals live on one 4 KiB page at the start of AXI SRAM; the stack goes anywhere, see
   kernel/src/process.rs. */
__app_ram = 0x24000000;

/* Separate segments, or the file would span the gap between flash and RAM. */
PHDRS
{
  flash PT_LOAD;
  ram PT_LOAD;
}

SECTIONS
{
  /* librecalc_abi::AppHeader */
  .header __app_flash :
  {
    __app_header = .;
    LONG(0x5041434C)             /* "LCAP" */
    LONG(1)                      /* schema */
    LONG(__app_end - __app_header)
    LONG(_start - __app_header)
    LONG(__app_name - __app_header)
    LONG(__app_icon - __app_header)
    LONG(0)                      /* icon size: 0 means a Graphite icon */
    LONG(__bss_end - __app_ram)
    LONG(0x5041434C)
    KEEP(*(.rodata.app_name))
    KEEP(*(.rodata.app_icon))
  } :flash
  .text : { *(.text .text.*) }
  .rodata : { *(.rodata .rodata.*) }
  .ARM.exidx : { *(.ARM.exidx .ARM.exidx.*) }
  . = ALIGN(4);
  __data_load = .;
  .data __app_ram : AT(__data_load)
  {
    __data_start = .;
    *(.data .data.*)
    . = ALIGN(4);
    __data_end = .;
  } :ram
  .bss (NOLOAD) :
  {
    *(.bss .bss.* COMMON)
    . = ALIGN(4);
    __bss_end = .;
  }
  __app_end = __data_load + SIZEOF(.data);
}

ASSERT(__bss_end <= 0x24001000, "app globals outgrew their 4 KiB page");
