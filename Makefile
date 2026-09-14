# SPDX-License-Identifier: GPL-3.0-only

BUILD ?= build
PYTHON ?= python3
RENODE ?= renode
CARGO ?= cargo
CROSS ?= arm-none-eabi-
OBJCOPY := $(CROSS)objcopy
SIZE := $(CROSS)size
CXX ?= c++
QT_HEADERS ?= $(shell qmake6 -query QT_INSTALL_HEADERS 2>/dev/null)
QT_LIBS ?= $(shell qmake6 -query QT_INSTALL_LIBS 2>/dev/null)

RUST_TARGET := thumbv7em-none-eabihf
CARGO_OUT := target/$(RUST_TARGET)/release
BOOTLOADER_ELF := $(BUILD)/librecalc-bootloader.elf
BOOTLOADER_BIN := $(BUILD)/librecalc-bootloader.bin
BOOTLOADER_DFU := $(BUILD)/librecalc-bootloader.dfu
KERNEL_ELF := $(BUILD)/librecalc-kernel.elf
KERNEL_BIN := $(BUILD)/librecalc-kernel.bin
KERNEL_DFU := $(BUILD)/librecalc-kernel.dfu
FIRMWARE_DFU := $(BUILD)/librecalc.dfu
STATE := emulator/state

.PHONY: all help clean bootloader kernel flash-virtual panel run-panel test-panel test-usb

all: bootloader kernel
	$(PYTHON) tools/dfuse.py pack $(FIRMWARE_DFU) \
		0x08000000:$(BOOTLOADER_BIN) 0x90000000:$(KERNEL_BIN)

help:
	@echo 'make                         build the bootloader, kernel, and combined DfuSe image'
	@echo 'make bootloader              build the bootloader artifacts'
	@echo 'make kernel                  build the kernel artifacts'
	@echo 'make flash-virtual IMAGE=x   write a DfuSe into Renode virtual flash'
	@echo 'make run-panel               start the local calculator window'
	@echo 'make test-panel              check and render the calculator window'
	@echo 'make test-usb                check an attached virtual calculator with PyUSB'
	@echo 'make clean                   remove generated files'

$(BUILD):
	mkdir -p $@

bootloader: | $(BUILD)
	$(CARGO) build --release --package librecalc-bootloader
	cp $(CARGO_OUT)/librecalc-bootloader $(BOOTLOADER_ELF)
	$(SIZE) $(BOOTLOADER_ELF)
	$(OBJCOPY) -O binary $(BOOTLOADER_ELF) $(BOOTLOADER_BIN)
	$(PYTHON) tools/dfuse.py pack $(BOOTLOADER_DFU) 0x08000000:$(BOOTLOADER_BIN)

kernel: | $(BUILD)
	$(CARGO) build --release --package librecalc-kernel
	cp $(CARGO_OUT)/librecalc-kernel $(KERNEL_ELF)
	$(SIZE) $(KERNEL_ELF)
	$(OBJCOPY) -O binary $(KERNEL_ELF) $(KERNEL_BIN)
	$(PYTHON) tools/dfuse.py pack $(KERNEL_DFU) 0x90000000:$(KERNEL_BIN)

flash-virtual:
	test -n "$(IMAGE)" || (echo 'usage: make flash-virtual IMAGE=path/to/image.dfu' >&2; exit 2)
	$(PYTHON) tools/dfuse.py flash-virtual "$(IMAGE)" $(STATE)

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
	rm -rf -- build target
