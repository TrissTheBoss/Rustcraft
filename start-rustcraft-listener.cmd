@echo off
setlocal
set "RUSTCRAFT_CONFIG=%~dp0rustcraft.conf"
set "RUSTCRAFT_LOG=%~dp0rustcraft.log"
"%~dp0target\release\rustcraft.exe" --listen
endlocal
