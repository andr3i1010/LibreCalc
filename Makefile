# SPDX-License-Identifier: GPL-3.0-only

BUILD ?= build
RENODE ?= renode
SYSROOT := $(shell rustc --print sysroot)
OBJCOPY := LD_LIBRARY_PATH=$(SYSROOT)/lib $(SYSROOT)/lib/rustlib/$(shell rustc --print host-tuple)/bin/rust-objcopy
LLD := LD_LIBRARY_PATH=$(SYSROOT)/lib $(SYSROOT)/lib/rustlib/$(shell rustc --print host-tuple)/bin/rust-lld -flavor gnu
CXX ?= c++

BOOTLOADER_BIN := $(BUILD)/librecalc-bootloader.bin
KERNEL_BIN := $(BUILD)/librecalc-kernel.bin
TRAMPOLINE_BIN := $(BUILD)/librecalc-bootloader-table.bin
APPS_BIN := $(BUILD)/apps.bin
LIBRARY_BIN := $(BUILD)/graphite-ui.bin
STATE ?= emulator/state
APPS := $(notdir $(wildcard apps/*))
BOOT ?= home

.PHONY: all help clean bootloader kernel flash-virtual panel run-panel

all: bootloader kernel
	python3 tools/pack_dfu.py $(BOOTLOADER_BIN) $(TRAMPOLINE_BIN) $(KERNEL_BIN) $(LIBRARY_BIN) $(APPS_BIN) $(BUILD)/librecalc.dfu

help:
	@echo 'make                         build the bootloader, kernel, and combined DFU'
	@echo 'make bootloader              build the bootloader DFU and raw artifacts'
	@echo 'make kernel                  build the apps and kernel; BOOT=<app> picks the app it boots,'
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
	cargo build --release --package graphite-ui-library
	$(LLD) -T tools/library.ld --gc-sections target/thumbv7em-none-eabihf/release/libgraphite_ui_library.a -o $(BUILD)/graphite-ui.elf
	$(OBJCOPY) -O binary $(BUILD)/graphite-ui.elf $(LIBRARY_BIN)
	cargo build --release $(addprefix --package librecalc-,$(APPS)) $(if $(CRASH),--features librecalc-hello/crash)
	: > $(APPS_BIN)
	for app in $(APPS); do \
		truncate -s %4096 $(APPS_BIN) && \
		address=$$((0x90100000 + $$(stat -c %s $(APPS_BIN)))) && \
		$(LLD) -r -u _start -u __app_name -u __app_icon target/thumbv7em-none-eabihf/release/liblibrecalc_$$app.a -o $(BUILD)/$$app.lca && \
		$(LLD) -T tools/app.ld --defsym=__app_flash=$$address --gc-sections $(BUILD)/$$app.lca -o $(BUILD)/$$app.elf && \
		$(OBJCOPY) -O binary $(BUILD)/$$app.elf $(BUILD)/$$app.bin && \
		cat $(BUILD)/$$app.bin >> $(APPS_BIN) || exit 1; \
	done
	$(OBJCOPY) --strip-all --keep-symbol=__app_header $(BUILD)/$(BOOT).elf $(BUILD)/boot.sym
	cargo build --release --package librecalc-kernel
	cp target/thumbv7em-none-eabihf/release/librecalc-kernel $(BUILD)/librecalc-kernel.elf
	$(OBJCOPY) -O binary $(BUILD)/librecalc-kernel.elf $(KERNEL_BIN)
