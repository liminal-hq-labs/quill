@echo off
rem Applies demo-settings.reg and the small page file inside the guest, then shuts it down so the disk is clean. Run from the setup CD: cmd /c D:\APPLY.CMD
rem (c) Copyright 2026 Liminal HQ, Scott Morris
rem SPDX-License-Identifier: Apache-2.0 OR MIT
set LOG=COM1
echo apply: start >> %LOG%
regedit /s %~dp0DEMO.REG
echo apply: regedit %ERRORLEVEL% >> %LOG%
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management" /v PagingFiles /t REG_MULTI_SZ /d "C:\pagefile.sys 64 64" /f >> %LOG%
reg query "HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management" /v PagingFiles >> %LOG%
reg query "HKLM\SYSTEM\CurrentControlSet\Services\bochsmp\Device0" >> %LOG%
reg query "HKCU\Control Panel\Desktop" /v Wallpaper >> %LOG%
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" >> %LOG%
echo apply: done >> %LOG%
shutdown -s -t 0
