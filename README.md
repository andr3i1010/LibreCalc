# LibreCalc

LibreCalc is an experimental, independent operating-system project for the
NumWorks N0120 calculator. It contains Rust bootloader and kernel scaffolds,
an N0120 hardware library, STM DfuSe tooling, a Renode hardware model, and a
Qt calculator panel.

The Rust entry points currently take ownership of the board and wait. The
bootloader does not start the kernel yet, and nothing produced here should
currently be flashed onto physical hardware.

## Workspace

```text
bootloader ─┐
            ├──> n0120-hardware
kernel ─────┘
```

- `bootloader/` is the internal-flash program.
- `kernel/` is the external-flash program.
- `hardware/` is the safe board-support library for all N0120 hardware.
- `emulator/` contains the Renode model and calculator panel.
- `tools/` contains host-side firmware tooling.

The bootloader and kernel are independent programs. Neither depends on the
other. New modules should be added only when they contain real code, and code
should move into a new shared crate only after both programs genuinely need it.

## Requirements

- Rustup
- GNU Make
- GNU Arm Embedded tools (`arm-none-eabi-objcopy` and `arm-none-eabi-size`)
- Python 3
- Renode (tested with 1.16.1)
- Qt 6 development files and `qmake6`

Rustup reads `rust-toolchain.toml` and installs the pinned compiler, formatter,
linter, editor engine, and Cortex-M target.

## Build

```sh
make
```

This creates:

- `build/librecalc-bootloader.{elf,bin,dfu}`
- `build/librecalc-kernel.{elf,bin,dfu}`
- `build/librecalc.dfu`, containing both programs

Build only one program with `make bootloader` or `make kernel`.

Fast Rust checks use Cargo directly:

```sh
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

The DfuSe tool has a standalone self-check:

```sh
python3 tools/dfuse.py self-test
```

## Emulator

Load the combined image into virtual flash, then launch the calculator panel:

```sh
make flash-virtual IMAGE=build/librecalc.dfu
make run-panel
```

`make test-panel` runs the panel's offscreen geometry, input, and rendering
check. LibreCalc's flashing command only writes the emulator's virtual flash
files. Flashing a physical calculator is intentionally left to `dfu-util`.

If Renode is not on `PATH`, set it explicitly:

```sh
RENODE=/path/to/renode make run-panel
```

Cargo stores compiler intermediates in `target/`. Final firmware images,
ELFs with debug symbols, and emulator outputs go in `build/`.

## Clean-room boundary

Firmware images, virtual flash contents, LCD captures, Ghidra projects,
decompiler output, and all build output are excluded from version control.
LibreCalc does not distribute NumWorks firmware, source code, or artwork.
Any necessary reverse engineering must be reduced to behavioral notes before
an independent implementation is written.

## License

LibreCalc is licensed under the GNU General Public License version 3 only.
See [LICENSE](LICENSE).

NumWorks and N0120 are used only to identify compatible hardware. LibreCalc
is not affiliated with or endorsed by NumWorks.
