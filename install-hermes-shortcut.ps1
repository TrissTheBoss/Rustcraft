$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$shortcutPath = Join-Path $env:APPDATA 'Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\Hermes Voxel Engine.lnk'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = Join-Path $root 'start-hermes.cmd'
$shortcut.WorkingDirectory = $root
$shortcut.Description = 'Start Hermes for Voxel Engine'
$shortcut.Save()
Write-Host "Created shortcut: $shortcutPath"
Write-Host "Windows may require you to pin or refresh the taskbar item manually."
