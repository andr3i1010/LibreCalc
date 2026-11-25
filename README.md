# LibreCalc

LibreCalc is an experimental, independent operating system project for the
NumWorks N0120 calculator.

## Installation
You need an exploit like [Wirehax](https://github.com/numcrack/wirehax) to unlock the stock bootloader and overwrite it with LibreCalc.

If you want to install LibreCalc using SWD, check https://blog.ayabusa.dev/2025/11/07/lets-create-an-os-for-the-numworks-a-different-approach-part-2/

## Workspace
- `bootloader/` is the bootloader that lives in the internal flash. Its goal is to boot the kernel from external flash, and to provide a DFU interface for flashing.
- `kernel/` is the kernel that lives in the external flash. It is responsible for providing safe access to the hardware for the userspace.
- `hardware/` is the shared hardware library.

## Requirements

- Rustup
- GNU Make
- Python 3 (emulator)
- Renode (tested with 1.16.1) (emulator)
- Qt 6 development files and `qmake6` (emulator)
- Optional virtual USB on Linux: `usbip` and `pkexec` (emulator)

Rustup reads `rust-toolchain.toml` and installs the pinned compiler, formatter,
linter, editor engine, and Cortex-M target.

## Build

```sh
make all
```

This creates:

- `build/librecalc-bootloader.{elf,bin}`
- `build/librecalc-bootloader-table.bin`
- `build/librecalc-bootloader.dfu`, containing the bootloader and its matching Epsilon call table.
- `build/librecalc-kernel.{elf,bin}`
- `build/librecalc.dfu`, containing all three images.

Build only the bootloader with `make bootloader`, or builtin apps + the kernel with `make kernel`.

Use `build/librecalc-bootloader.dfu` to update the bootloader and call table together
while keeping the external firmware.

Older bootloaders reject the call-table erase at `0x0807FC00`. To update one:

1. In DFU mode, install the new body:
   ```sh
   dfu-util -d 0483:a291 -a 0 -s 0x08000000 -D build/librecalc-bootloader.bin
   ```
2. Press BACK and select DFU again so the new erase handler runs.
3. Install the complete bootloader image:
   ```sh
   dfu-util -d 0483:a291 -a 0 -D build/librecalc-bootloader.dfu
   ```
4. After both elements finish downloading, press BACK to reboot.

Fast Rust checks use Cargo directly:

```sh
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

## License

LibreCalc is licensed under the GNU General Public License version 3 only.
See [LICENSE](LICENSE).

The LibreCalc Project is not affiliated with, or endorsed by NumWorks SAS.