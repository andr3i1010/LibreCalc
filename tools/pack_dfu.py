#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only

import struct
import sys
import zlib
from pathlib import Path

assert len(sys.argv) in (4, 5), "Usage: pack_dfu.py BOOTLOADER TABLE [KERNEL] OUTPUT"
bootloader, trampoline, *kernel, output = map(Path, sys.argv[1:])
assert bootloader.stat().st_size <= 0x7FA00
assert trampoline.stat().st_size == 13 * 4
assert all(path.stat().st_size <= 0x3F0000 for path in kernel)
elements = b""
for address, path in zip((0x08000000, 0x0807FC00, 0x90000000), (bootloader, trampoline, *kernel)):
    data = path.read_bytes()
    elements += struct.pack("<II", address, len(data)) + data

target = struct.pack(
    "<6sBI255sII", b"Target", 0, 1, b"LibreCalc N0120", len(elements), 2 + len(kernel)
) + elements
image = struct.pack("<5sBIB", b"DfuSe", 1, 11 + len(target), 1) + target
image += struct.pack("<HHHH3sB", 0, 0xA291, 0x0483, 0x011A, b"UFD", 16)
output.write_bytes(image + struct.pack("<I", zlib.crc32(image) ^ 0xFFFFFFFF))
