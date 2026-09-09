# SPDX-License-Identifier: GPL-3.0-only

BUILD ?= build
PYTHON ?= python3
RENODE ?= renode
CROSS ?= arm-none-eabi-
CC := $(CROSS)gcc
OBJCOPY := $(CROSS)objcopy
SIZE := $(CROSS)size
CXX ?= c++
QT_HEADERS ?= $(shell qmake6 -query QT_INSTALL_HEADERS 2>/dev/null)
QT_LIBS ?= $(shell qmake6 -query QT_INSTALL_LIBS 2>/dev/null)

CPUFLAGS := -mcpu=cortex-m7 -mthumb -mfloat-abi=hard -mfpu=fpv5-d16
CFLAGS := $(CPUFLAGS) -std=c11 -Os -ffreestanding -fno-builtin \
	-ffunction-sections -fdata-sections -Wall -Wextra -Werror
LDFLAGS := -nostdlib -Wl,--gc-sections,-Map=$(BUILD)/bootloader.map \
	-Wl,-T,boot/n0120.ld

INTERNAL_DFU := $(BUILD)/librecalc-internal.dfu
STATE := emulator/state

.PHONY: all help clean test flash-virtual run panel run-panel test-panel

all: $(INTERNAL_DFU)

help:
	@echo 'make                         build the internal-flash DfuSe image'
	@echo 'make test                    build and run parser/storage checks'
	@echo 'make flash-virtual IMAGE=x   write a DfuSe into Renode virtual flash'
	@echo 'make run                     start Renode with emulator/state/'
	@echo 'make panel                   build the local calculator window'
	@echo 'make run-panel               start the local calculator window'
	@echo 'make test-panel              check and render the calculator window'

$(BUILD):
	mkdir -p $@

$(BUILD)/bootloader.elf: boot/bootloader.c boot/n0120.ld | $(BUILD)
	$(CC) $(CFLAGS) $< $(LDFLAGS) -o $@
	$(SIZE) $@

$(BUILD)/bootloader.bin: $(BUILD)/bootloader.elf
	$(OBJCOPY) -O binary $< $@

$(INTERNAL_DFU): $(BUILD)/bootloader.bin tools/dfuse.py
	$(PYTHON) tools/dfuse.py pack $@ 0x08000000:$<

test: all
	$(PYTHON) tools/dfuse.py self-test
	$(PYTHON) tools/dfuse.py inspect $(INTERNAL_DFU)

flash-virtual:
	test -n "$(IMAGE)" || (echo 'usage: make flash-virtual IMAGE=path/to/image.dfu' >&2; exit 2)
	$(PYTHON) tools/dfuse.py flash "$(IMAGE)" $(STATE)

run:
	test -f $(STATE)/internal.bin || (echo 'flash a DfuSe first' >&2; exit 2)
	test -f $(STATE)/external.bin || (echo 'flash a DfuSe first' >&2; exit 2)
	LIBRECALC_ROOT=$(CURDIR) $(RENODE) emulator/run.resc

panel: $(BUILD)/librecalc-panel

$(BUILD)/librecalc-panel: emulator/panel.cpp | $(BUILD)
	test -n "$(QT_HEADERS)" -a -n "$(QT_LIBS)" || (echo 'Qt 6 headers/libraries not found; set QT_HEADERS and QT_LIBS' >&2; exit 2)
	$(CXX) -std=c++17 -I$(QT_HEADERS) -I$(QT_HEADERS)/QtCore -I$(QT_HEADERS)/QtGui -I$(QT_HEADERS)/QtWidgets -I$(QT_HEADERS)/QtNetwork \
		emulator/panel.cpp -L$(QT_LIBS) -Wl,-rpath,$(QT_LIBS) -lQt6Widgets -lQt6Network -lQt6Gui -lQt6Core -o $@

run-panel: panel
	LIBRECALC_ROOT=$(CURDIR) RENODE=$(RENODE) $(BUILD)/librecalc-panel

test-panel: panel
	QT_QPA_PLATFORM=offscreen LIBRECALC_ROOT=$(CURDIR) $(BUILD)/librecalc-panel --self-test

clean:
	rm -rf -- $(BUILD)
