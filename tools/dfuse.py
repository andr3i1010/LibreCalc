#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only

"""Small, strict STM DfuSe packer and virtual-flash writer for N0120."""

import argparse
import binascii
import os
import struct
import tempfile
from pathlib import Path


REGIONS = {
    "internal": (0x08000000, 0x80000),
    "external": (0x90000000, 0x800000),
}
VID = 0x0483
PID = 0xA291
DFU_VERSION = 0x011A


def _crc(data):
    return binascii.crc32(data) ^ 0xFFFFFFFF


def _region(address, size):
    end = address + size
    for name, (base, length) in REGIONS.items():
        if base <= address and end <= base + length:
            return name, address - base
    raise ValueError(f"element 0x{address:08x}+0x{size:x} is outside N0120 flash")


def _check_element(elements, address, payload):
    if not payload:
        raise ValueError(f"element at 0x{address:08x} is empty")
    _region(address, len(payload))
    end = address + len(payload)
    for old_address, old_payload in elements:
        old_end = old_address + len(old_payload)
        if old_address < end and address < old_end:
            raise ValueError(f"elements overlap at 0x{address:08x}")


def parse(path):
    data = Path(path).read_bytes()
    if len(data) < 11 + 16 or data[:5] != b"DfuSe":
        raise ValueError("not a DfuSe image")

    version, image_size, target_count = struct.unpack_from("<BIB", data, 5)
    if version != 1:
        raise ValueError(f"unsupported DfuSe version {version}")
    if image_size != len(data) - 16:
        raise ValueError("DfuSe image size does not match file length")

    suffix_offset = len(data) - 16
    device, pid, vid, dfu_version, signature, suffix_length, stored_crc = struct.unpack_from(
        "<HHHH3sBI", data, suffix_offset
    )
    if (vid, pid) != (VID, PID):
        raise ValueError(f"unexpected device {vid:04x}:{pid:04x}")
    if signature != b"UFD" or suffix_length != 16 or dfu_version != DFU_VERSION:
        raise ValueError("invalid DfuSe suffix")
    if stored_crc != _crc(data[:-4]):
        raise ValueError("DfuSe CRC mismatch")

    elements = []
    offset = 11
    for target_index in range(target_count):
        if offset + 274 > suffix_offset:
            raise ValueError(f"truncated target {target_index}")
        target, alternate, named, raw_name, target_size, element_count = struct.unpack_from(
            "<6sBI255sII", data, offset
        )
        if target != b"Target":
            raise ValueError(f"invalid target signature {target!r}")
        offset += 274
        target_end = offset + target_size
        if target_end > suffix_offset:
            raise ValueError(f"target {target_index} exceeds image")
        target_start = offset
        for element_index in range(element_count):
            if offset + 8 > target_end:
                raise ValueError(f"truncated element {target_index}:{element_index}")
            address, size = struct.unpack_from("<II", data, offset)
            offset += 8
            if offset + size > target_end:
                raise ValueError(f"truncated payload {target_index}:{element_index}")
            payload = data[offset : offset + size]
            offset += size
            _check_element([(old[0], old[1]) for old in elements], address, payload)
            elements.append((address, payload))
        if offset != target_end or offset - target_start != target_size:
            raise ValueError(f"target {target_index} size mismatch")

    if offset != suffix_offset:
        raise ValueError("trailing bytes before DfuSe suffix")
    return {"device": device, "elements": elements, "targets": target_count}


def _image(elements):
    checked = []
    for address, payload in elements:
        _check_element(checked, address, payload)
        checked.append((address, payload))
    body = bytearray()
    target_body = bytearray()
    for address, payload in elements:
        target_body += struct.pack("<II", address, len(payload)) + payload
    target = struct.pack(
        "<6sBI255sII", b"Target", 0, 1, b"LibreCalc N0120".ljust(255, b"\0"), len(target_body), len(elements)
    )
    body += struct.pack("<5sBIB", b"DfuSe", 1, 11 + len(target) + len(target_body), 1)
    body += target + target_body
    suffix = struct.pack("<HHHH3sB", 0, PID, VID, DFU_VERSION, b"UFD", 16)
    return bytes(body + suffix + struct.pack("<I", _crc(body + suffix)))


def _atomic_write(path, data):
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def flash(image, state):
    parsed = parse(image)
    state = Path(state)
    state.mkdir(parents=True, exist_ok=True)
    storage = {}
    for name, (_, length) in REGIONS.items():
        path = state / f"{name}.bin"
        if path.exists():
            storage[name] = bytearray(path.read_bytes())
            if len(storage[name]) != length:
                raise ValueError(f"{path} is not exactly {length} bytes")
        else:
            storage[name] = bytearray(b"\xff" * length)
    for address, payload in parsed["elements"]:
        name, offset = _region(address, len(payload))
        storage[name][offset : offset + len(payload)] = payload
    for name, payload in storage.items():
        _atomic_write(state / f"{name}.bin", payload)
    print(f"flashed {image} into {state} ({len(parsed['elements'])} elements)")


def inspect(image):
    parsed = parse(image)
    print(f"device: {VID:04x}:{PID:04x}")
    print(f"targets: {parsed['targets']}")
    for address, payload in parsed["elements"]:
        print(f"element: 0x{address:08x} +0x{len(payload):x}")


def self_test():
    from tempfile import TemporaryDirectory

    with TemporaryDirectory() as directory:
        root = Path(directory)
        original = root / "original.dfu"
        original.write_bytes(_image([(0x08000000, b"boot"), (0x90000000, b"slot-a")]))
        assert len(parse(original)["elements"]) == 2
        state = root / "state"
        flash(original, state)
        assert (state / "internal.bin").read_bytes()[:4] == b"boot"
        assert (state / "external.bin").read_bytes()[:6] == b"slot-a"
        external = (state / "external.bin").read_bytes()
        replacement = root / "replacement.dfu"
        replacement.write_bytes(_image([(0x08000001, b"new")]))
        flash(replacement, state)
        assert (state / "external.bin").read_bytes() == external
        damaged = bytearray(original.read_bytes())
        damaged[20] ^= 1
        damaged_path = root / "damaged.dfu"
        damaged_path.write_bytes(damaged)
        try:
            parse(damaged_path)
        except ValueError:
            pass
        else:
            raise AssertionError("damaged image was accepted")
    print("dfuse self-test: ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    pack = commands.add_parser("pack")
    pack.add_argument("output")
    pack.add_argument("elements", nargs="+")
    inspect_parser = commands.add_parser("inspect")
    inspect_parser.add_argument("image")
    flash_parser = commands.add_parser("flash")
    flash_parser.add_argument("image")
    flash_parser.add_argument("state")
    commands.add_parser("self-test")
    args = parser.parse_args()
    try:
        if args.command == "pack":
            elements = []
            for specification in args.elements:
                address, separator, filename = specification.partition(":")
                if not separator:
                    raise ValueError(f"invalid element {specification!r}; use ADDRESS:FILE")
                elements.append((int(address, 0), Path(filename).read_bytes()))
            Path(args.output).write_bytes(_image(elements))
            inspect(args.output)
        elif args.command == "inspect":
            inspect(args.image)
        elif args.command == "flash":
            flash(args.image, args.state)
        else:
            self_test()
    except (OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
