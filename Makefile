# SPDX-License-Identifier: GPL-3.0-only

BUILD ?= build
RENODE ?= renode
CROSS ?= arm-none-eabi-
CXX ?= c++
QT_HEADERS ?= $(shell qmake6 -query QT_INSTALL_HEADERS 2>/dev/null)
QT_LIBS ?= $(shell qmake6 -query QT_INSTALL_LIBS 2>/dev/null)

BOOTLOADER_BIN := $(BUILD)/librecalc-bootloader.bin
KERNEL_BIN := $(BUILD)/librecalc-kernel.bin
TRAMPOLINE_BIN := $(BUILD)/librecalc-bootloader-table.bin
STATE ?= emulator/state

.PHONY: all help clean bootloader kernel flash-virtual panel run-panel test-panel test-usb test-lcd test-syscalls

all: bootloader kernel
	python3 tools/pack_dfu.py $(BOOTLOADER_BIN) $(TRAMPOLINE_BIN) $(KERNEL_BIN) $(BUILD)/librecalc.dfu

help:
	@echo 'make                         build the bootloader, kernel, and combined DFU'
	@echo 'make bootloader              build the bootloader artifacts'
	@echo 'make kernel                  build the kernel artifacts'
	@echo 'make flash-virtual           write the current build into Renode virtual flash'
	@echo 'make run-panel               start the local calculator window'
	@echo 'make test-panel              check and render the calculator window'
	@echo 'make test-lcd                check final LCD frame delivery'
	@echo 'make test-usb                check virtual USB registers'
	@echo 'make test-syscalls           check syscall routing and wrappers'
	@echo 'make clean                   remove generated files'

$(BUILD):
	mkdir -p $@

bootloader kernel: | $(BUILD)
	cargo build --release --package librecalc-$@
	cp target/thumbv7em-none-eabihf/release/librecalc-$@ $(BUILD)/librecalc-$@.elf
	$(CROSS)size $(BUILD)/librecalc-$@.elf
	$(CROSS)objcopy -O binary $(if $(filter bootloader,$@),--remove-section=.trampoline,) $(BUILD)/librecalc-$@.elf $(BUILD)/librecalc-$@.bin
	$(if $(filter bootloader,$@),$(CROSS)objcopy -O binary --only-section=.trampoline $(BUILD)/librecalc-$@.elf $(TRAMPOLINE_BIN),@true)

flash-virtual: bootloader kernel
	mkdir -p $(STATE)
	test -f $(STATE)/internal.bin || cp emulator/state/internal.bin $(STATE)/internal.bin
	test -f $(STATE)/external.bin || cp emulator/state/external.bin $(STATE)/external.bin
	python3 tools/flash_virtual.py $(STATE) $(BOOTLOADER_BIN) $(TRAMPOLINE_BIN) $(KERNEL_BIN)

panel: $(BUILD)/librecalc-panel

$(BUILD)/librecalc-panel: emulator/panel.cpp | $(BUILD)
	test -n "$(QT_HEADERS)" -a -n "$(QT_LIBS)" || (echo 'Qt 6 headers/libraries not found; set QT_HEADERS and QT_LIBS' >&2; exit 2)
	$(CXX) -std=c++17 -I$(QT_HEADERS) -I$(QT_HEADERS)/QtCore -I$(QT_HEADERS)/QtGui -I$(QT_HEADERS)/QtWidgets -I$(QT_HEADERS)/QtNetwork \
		emulator/panel.cpp -L$(QT_LIBS) -Wl,-rpath,$(QT_LIBS) -lQt6Widgets -lQt6Network -lQt6Gui -lQt6Core -o $@

run-panel:
	LIBRECALC_ROOT=$(CURDIR) LIBRECALC_STATE=$(CURDIR)/$(STATE) RENODE=$(RENODE) $(BUILD)/librecalc-panel

clean:
	rm -rf -- build target
