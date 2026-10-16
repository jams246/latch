export GPUI_FXC_PATH ?= $(shell powershell.exe -NoProfile -Command "Get-ChildItem -Path ([IO.Path]::Combine([Environment]::GetFolderPath('ProgramFilesX86'), 'Windows Kits/10/bin/*/x64/fxc.exe')) -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName")

.PHONY: build\:production build\:msi

build\:production:
	powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File scripts/build-production.ps1

build\:msi:
	powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File scripts/build-msi.ps1
