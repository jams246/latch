ifeq ($(OS),Windows_NT)
export GPUI_FXC_PATH ?= $(shell powershell.exe -NoProfile -Command "Get-ChildItem -Path ([IO.Path]::Combine([Environment]::GetFolderPath('ProgramFilesX86'), 'Windows Kits/10/bin/*/x64/fxc.exe')) -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName")
endif

.PHONY: build\:production

build\:production:
	cargo build --release --locked --bin latch
