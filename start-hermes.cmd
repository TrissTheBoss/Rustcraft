@echo off
setlocal
set "ROOT=%~dp0"
start "Hermes Worker" /min powershell.exe -NoProfile -WindowStyle Hidden -Command "& pythonw '%ROOT%hermes\hermes_worker.py'"
start "Hermes Terminal" cmd.exe /k "cd /d %ROOT% && python hermes\hermes_terminal.py"
endlocal
