@echo off
rem Applies every polish registry file next to this script (PL_MOUSE.REG on the setup CD, polish/mouse.reg in the repository) inside the guest.
rem Called by apply-settings.cmd on the setup CD as POLISH.CMD, or by hand: cmd /c D:\POLISH.CMD
rem (c) Copyright 2026 Liminal HQ, Scott Morris
rem SPDX-License-Identifier: Apache-2.0 OR MIT
echo polish: start >> COM1
for %%f in (%~dp0PL_*.REG) do (
	regedit /s "%%f"
	echo polish: %%f %ERRORLEVEL% >> COM1
)
reg query "HKCU\Control Panel\Mouse" /v MouseSpeed >> COM1
echo polish: done >> COM1
