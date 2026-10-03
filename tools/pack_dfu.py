#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only

import struct
import sys
import zlib
from pathlib import Path

bootloader, kernel, output = map(Path, sys.argv[1:])
elements = b""
for address, path in ((0x08000000, bootloader), (0x90000000, kernel)):
    data = path.read_bytes()
    elements += struct.pack("<II", address, len(data)) + data

target = struct.pack(
    "<6sBI255sII", b"Target", 0, 1, b"LibreCalc N0120", len(elements), 2
) + elements
image = struct.pack("<5sBIB", b"DfuSe", 1, 11 + len(target) + 16, 1) + target
image += struct.pack("<HHHH3sB", 0, 0xA291, 0x0483, 0x011A, b"UFD", 16)
output.write_bytes(image + struct.pack("<I", zlib.crc32(image) ^ 0xFFFFFFFF))
