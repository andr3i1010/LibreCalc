# LibreCalc

LibreCalc is an experimental, independent operating-system project for the
NumWorks N0120 calculator. The repository currently contains a clean-room
bootloader base, STM DfuSe tooling, a Renode hardware model, and a functional
Qt calculator panel.

The replacement bootloader is currently only a hello-world scaffold, and the
kernel is not implemented yet. Nothing in this repository should currently be
flashed onto physical hardware.

## Requirements

- GNU Make
- GNU Arm Embedded toolchain (`arm-none-eabi-gcc` and `objcopy`)
- Python 3
- Renode (tested with 1.16.1)
- Qt 6 development files and `qmake6`

## Build

```sh
make
make test
```

This produces `build/librecalc-internal.dfu`, containing only the minimal
internal-flash bootloader scaffold.

## Emulator

Load a locally supplied N0120 DfuSe image into the virtual flash, then launch
the calculator panel:

```sh
make flash-virtual IMAGE=/path/to/firmware.dfu
make run-panel
```

If Renode is not on `PATH`, set it explicitly:

```sh
RENODE=/path/to/renode make run-panel
```

`make run` starts Renode without the calculator panel. `make test-panel`
checks the panel and writes a deterministic preview under `build/`.

Firmware images, virtual flash contents, LCD captures, Ghidra projects, and
all build output are intentionally excluded from version control. LibreCalc
does not distribute NumWorks firmware, source code, or artwork.

## License

LibreCalc is licensed under the GNU General Public License version 3 only.
See [LICENSE](LICENSE).

NumWorks and N0120 are used only to identify compatible hardware. LibreCalc
is not affiliated with or endorsed by NumWorks.
