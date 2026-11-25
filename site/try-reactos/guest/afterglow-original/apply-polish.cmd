@echo off
rem Demo polish only (no version watermark, smooth wallpaper): imports polish.reg. Run from a CD that holds POLISH.REG next to
rem this script, then log off and on: cmd /c D:\APOLISH.CMD
rem (c) Copyright 2026 Liminal HQ, Scott Morris
rem SPDX-License-Identifier: Apache-2.0 OR MIT
regedit /s %~dp0POLISH.REG
echo polish: regedit %ERRORLEVEL% >> COM1
reg query "HKCU\Control Panel\Desktop" /v PaintDesktopVersion >> COM1
