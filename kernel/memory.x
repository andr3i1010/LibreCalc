/* SPDX-License-Identifier: GPL-3.0-only */

MEMORY
{
  FLASH : ORIGIN = 0x90000000, LENGTH = 8M
  RAM   : ORIGIN = 0x24000000, LENGTH = 256K
}

/* Leave one aligned KiB for the STM32H725 vector table. */
_stext = ORIGIN(FLASH) + 0x400;
