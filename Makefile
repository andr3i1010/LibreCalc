# SPDX-License-Identifier: GPL-3.0-only

BUILD ?= build
RENODE ?= renode
SYSROOT := $(shell rustc --print sysroot)
OBJCOPY := LD_LIBRARY_PATH=$(SYSROOT)/lib $(SYSROOT)/lib/rustlib/$(shell rustc --print host-tuple)/bin/rust-objcopy
LLD := LD_LIBRARY_PATH=$(SYSROOT)/lib $(SYSROOT)/lib/rustlib/$(shell rustc --print host-tuple)/bin/rust-lld -flavor gnu
CXX ?= c++
QT_HEADERS ?= $(shell qmake6 -query QT_INSTALL_HEADERS 2>/dev/null)
QT_LIBS ?= $(shell qmake6 -query QT_INSTALL_LIBS 2>/dev/null)

BOOTLOADER_BIN := $(BUILD)/librecalc-bootloader.bin
KERNEL_BIN := $(BUILD)/librecalc-kernel.bin
TRAMPOLINE_BIN := $(BUILD)/librecalc-bootloader-table.bin
HOME_BIN := $(BUILD)/home.bin
STATE ?= emulator/state

.PHONY: all help clean bootloader kernel flash-virtual panel run-panel

all: bootloader kernel
	python3 tools/pack_dfu.py $(BOOTLOADER_BIN) $(TRAMPOLINE_BIN) $(KERNEL_BIN) $(HOME_BIN) $(BUILD)/librecalc.dfu

help:
	@echo 'make                         build the bootloader, kernel, and combined DFU'
	@echo 'make bootloader              build the bootloader DFU and raw artifacts'
	@echo 'make kernel                  build home and kernel; CRASH=1 builds the crash test'
	@echo 'make clean                   remove generated files'

$(BUILD):
	mkdir -p $@

bootloader: | $(BUILD)
	cargo build --release --package librecalc-bootloader
	cp target/thumbv7em-none-eabihf/release/librecalc-bootloader $(BUILD)/librecalc-bootloader.elf
	$(OBJCOPY) -O binary --remove-section=.trampoline $(BUILD)/librecalc-bootloader.elf $(BOOTLOADER_BIN)
	$(OBJCOPY) -O binary --only-section=.trampoline $(BUILD)/librecalc-bootloader.elf $(TRAMPOLINE_BIN)
	python3 tools/pack_dfu.py $(BOOTLOADER_BIN) $(TRAMPOLINE_BIN) $(BUILD)/librecalc-bootloader.dfu

kernel: | $(BUILD)
	cargo build --release --package librecalc-hello $(if $(CRASH),--features crash)
	$(LLD) -r -u _start -u __app_name target/thumbv7em-none-eabihf/release/liblibrecalc_hello.a -o $(BUILD)/hello.lca
	$(LLD) -T tools/app.ld --defsym=__app_flash=0x90100000 --gc-sections $(BUILD)/hello.lca -o $(BUILD)/home.elf
	$(OBJCOPY) -O binary $(BUILD)/home.elf $(HOME_BIN)
	$(OBJCOPY) --strip-all --keep-symbol=__app_header $(BUILD)/home.elf $(BUILD)/home.sym
	cargo build --release --package librecalc-kernel
	cp target/thumbv7em-none-eabihf/release/librecalc-kernel $(BUILD)/librecalc-kernel.elf
	$(OBJCOPY) -O binary $(BUILD)/librecalc-kernel.elf $(KERNEL_BIN)
