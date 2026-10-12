#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only

import struct
import sys
import zlib
from pathlib import Path

bootloader, trampoline, kernel, output = map(Path, sys.argv[1:])
assert bootloader.stat().st_size <= 0x7FA00
assert trampoline.stat().st_size == 13 * 4
assert kernel.stat().st_size <= 0x3F0000
elements = b""
for address, path in ((0x08000000, bootloader), (0x0807FC00, trampoline), (0x90000000, kernel)):
    data = path.read_bytes()
    elements += struct.pack("<II", address, len(data)) + data

target = struct.pack(
    "<6sBI255sII", b"Target", 0, 1, b"LibreCalc N0120", len(elements), 3
) + elements
image = struct.pack("<5sBIB", b"DfuSe", 1, 11 + len(target) + 16, 1) + target
image += struct.pack("<HHHH3sB", 0, 0xA291, 0x0483, 0x011A, b"UFD", 16)
output.write_bytes(image + struct.pack("<I", zlib.crc32(image) ^ 0xFFFFFFFF))
