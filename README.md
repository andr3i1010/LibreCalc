# LibreCalc

LibreCalc is an experimental, independent operating-system project for the
NumWorks N0120 calculator. It contains Rust bootloader and kernel scaffolds,
an N0120 hardware library, a tiny DFU packer, a Renode hardware model, and a
Qt calculator panel.

The Rust entry points currently take ownership of the board and wait. The
bootloader does not start the kernel yet, and nothing produced here should
currently be flashed onto physical hardware.

## Workspace
- `bootloader/` is the bootloader that lives in the internal flash. Its goal is to boot the kernel from external flash, and to provide a DFU interface for flashing.
- `kernel/` is the kernel that lives in the external flash. It is responsible for providing safe access to the hardware for the userspace.
- `hardware/` is the shared hardware library.
- `emulator/` contains the Renode model and calculator panel.
- `tools/pack_dfu.py` combines the bootloader and kernel into one DFU file.

## Requirements

- Rustup
- GNU Make
- GNU Arm Embedded tools (`arm-none-eabi-objcopy` and `arm-none-eabi-size`)
- Python 3
- Renode (tested with 1.16.1)
- Qt 6 development files and `qmake6`
- Optional virtual USB on Linux: `usbip` and `pkexec`

Rustup reads `rust-toolchain.toml` and installs the pinned compiler, formatter,
linter, editor engine, and Cortex-M target.

## Build

```sh
make all
```

This creates:

- `build/librecalc-bootloader.{elf,bin}`
- `build/librecalc-kernel.{elf,bin}`
- `build/librecalc.dfu`, containing both binaries.

Build only one program with `make bootloader` or `make kernel`.

Fast Rust checks use Cargo directly:

```sh
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

## Emulator

Write the current build into virtual flash, then launch the calculator panel:

```sh
make flash-virtual
make run-panel
```

If Renode is not on `PATH`, set it explicitly:

```sh
RENODE=/path/to/renode make run-panel
```

Cargo stores compiler intermediates in `target/`. Final firmware images,
ELFs with debug symbols, and emulator outputs go in `build/`.

## License

LibreCalc is licensed under the GNU General Public License version 3 only.
See [LICENSE](LICENSE).

The LibreCalc Project is not affiliated with, or endorsed by NumWorks SAS.
