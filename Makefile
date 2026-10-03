# SPDX-License-Identifier: GPL-3.0-only

BUILD ?= build
RENODE ?= renode
CROSS ?= arm-none-eabi-
CXX ?= c++
QT_HEADERS ?= $(shell qmake6 -query QT_INSTALL_HEADERS 2>/dev/null)
QT_LIBS ?= $(shell qmake6 -query QT_INSTALL_LIBS 2>/dev/null)

BOOTLOADER_BIN := $(BUILD)/librecalc-bootloader.bin
KERNEL_BIN := $(BUILD)/librecalc-kernel.bin
STATE := emulator/state

.PHONY: all help clean bootloader kernel flash-virtual panel run-panel test-panel test-usb test-lcd test-keypad

all: bootloader kernel
	python3 tools/pack_dfu.py $(BOOTLOADER_BIN) $(KERNEL_BIN) $(BUILD)/librecalc.dfu

help:
	@echo 'make                         build the bootloader, kernel, and combined DFU'
	@echo 'make bootloader              build the bootloader artifacts'
	@echo 'make kernel                  build the kernel artifacts'
	@echo 'make flash-virtual           write the current build into Renode virtual flash'
	@echo 'make run-panel               start the local calculator window'
	@echo 'make test-panel              check and render the calculator window'
	@echo 'make test-lcd                check final LCD frame delivery'
	@echo 'make test-keypad             check all calculator keys in Renode'
	@echo 'make test-usb                check virtual USB registers'
	@echo 'make clean                   remove generated files'

$(BUILD):
	mkdir -p $@

bootloader kernel: | $(BUILD)
	cargo build --release --package librecalc-$@
	cp target/thumbv7em-none-eabihf/release/librecalc-$@ $(BUILD)/librecalc-$@.elf
	$(CROSS)size $(BUILD)/librecalc-$@.elf
	$(CROSS)objcopy -O binary $(BUILD)/librecalc-$@.elf $(BUILD)/librecalc-$@.bin

flash-virtual: bootloader kernel
	mkdir -p $(STATE)
	$(CROSS)objcopy -I binary -O binary --gap-fill=0xff --pad-to=0x80000 $(BOOTLOADER_BIN) $(STATE)/internal.bin
	$(CROSS)objcopy -I binary -O binary --gap-fill=0xff --pad-to=0x800000 $(KERNEL_BIN) $(STATE)/external.bin

panel: $(BUILD)/librecalc-panel

$(BUILD)/librecalc-panel: emulator/panel.cpp | $(BUILD)
	test -n "$(QT_HEADERS)" -a -n "$(QT_LIBS)" || (echo 'Qt 6 headers/libraries not found; set QT_HEADERS and QT_LIBS' >&2; exit 2)
	$(CXX) -std=c++17 -I$(QT_HEADERS) -I$(QT_HEADERS)/QtCore -I$(QT_HEADERS)/QtGui -I$(QT_HEADERS)/QtWidgets -I$(QT_HEADERS)/QtNetwork \
		emulator/panel.cpp -L$(QT_LIBS) -Wl,-rpath,$(QT_LIBS) -lQt6Widgets -lQt6Network -lQt6Gui -lQt6Core -o $@

run-panel: panel
	LIBRECALC_ROOT=$(CURDIR) RENODE=$(RENODE) $(BUILD)/librecalc-panel

test-panel: panel
	QT_QPA_PLATFORM=offscreen LIBRECALC_ROOT=$(CURDIR) $(BUILD)/librecalc-panel --self-test

test-usb: | $(BUILD)
	$(RENODE) --disable-xwt --console --plain emulator/test_usb.resc 2>&1 | tee $(BUILD)/usb-test.log
	grep -q '^USB_SELF_TEST_OK' $(BUILD)/usb-test.log

test-lcd: | $(BUILD)
	$(RENODE) --disable-xwt --console --plain emulator/test_lcd.resc 2>&1 | tee $(BUILD)/lcd-test.log
	grep -q '^LCD_SELF_TEST_OK' $(BUILD)/lcd-test.log

test-keypad: bootloader
	$(RENODE) --disable-xwt --console --plain emulator/test_keypad.resc 2>&1 | tee $(BUILD)/keypad-test.log
	grep -q '^KEYPAD_SELF_TEST_OK' $(BUILD)/keypad-test.log

clean:
	rm -rf -- build target
