#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only

import sys
from pathlib import Path

state, bootloader, table, kernel = map(Path, sys.argv[1:])
code, services, image = bootloader.read_bytes(), table.read_bytes(), kernel.read_bytes()
assert len(code) <= 0x7FA00 and len(services) == 52 and len(image) <= 0x3F0000

state.mkdir(parents=True, exist_ok=True)
internal_path, external_path = state / "internal.bin", state / "external.bin"
internal = bytearray(internal_path.read_bytes() if internal_path.exists() else b"\xff" * 0x80000)
external = bytearray(external_path.read_bytes() if external_path.exists() else b"\xff" * 0x800000)
assert len(internal) <= 0x80000 and len(external) <= 0x800000
internal.extend(b"\xff" * (0x80000 - len(internal)))
external.extend(b"\xff" * (0x800000 - len(external)))

internal[:len(code)] = code
internal[0x7FC00:0x7FC00 + len(services)] = services
external[:len(image)] = image
internal_path.write_bytes(internal)
external_path.write_bytes(external)
