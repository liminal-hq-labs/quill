@echo off
rem Imports afterglow-original.reg (style, caption metrics, pointer scheme) and polish.reg (no version watermark, smooth
rem wallpaper), logs the result to the serial port and shuts the guest down so the disk is clean.
rem Run from the test CD: cmd /c D:\APPLY.CMD
rem (c) Copyright 2026 Liminal HQ, Scott Morris
rem SPDX-License-Identifier: Apache-2.0 OR MIT
regedit /s %~dp0AGORIG.REG
echo apply: style regedit %ERRORLEVEL% >> COM1
regedit /s %~dp0POLISH.REG
echo apply: polish regedit %ERRORLEVEL% >> COM1
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\ThemeManager" /v DllName >> COM1
reg query "HKCU\Control Panel\Desktop\WindowMetrics" >> COM1
reg query "HKCU\Control Panel\Desktop" /v PaintDesktopVersion >> COM1
reg query "HKCU\Control Panel\Desktop" /v Wallpaper >> COM1
reg query "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows" /v DisplayVersion >> COM1
reg query "HKCU\Control Panel\Cursors" >> COM1
shutdown -s -t 0
