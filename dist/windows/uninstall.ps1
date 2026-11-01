# This script removes the GameController that was installed with install.ps1. With -Purge, the
# user's configuration and logs are removed, too; otherwise they are kept.

param(
    [switch]$Purge
)

$ErrorActionPreference = "Stop"

$Directory = $PSScriptRoot
$Registry = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\GameController"
$Shortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\GameController.lnk"

Remove-Item -Force -ErrorAction SilentlyContinue $Shortcut
Remove-Item -Force -ErrorAction SilentlyContinue $Registry
Get-NetFirewallRule -DisplayName "GameController" -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction SilentlyContinue

if ($Purge) {
    $UserDirectory = Join-Path $env:LOCALAPPDATA "GameController"
    if (Test-Path $UserDirectory) {
        if ((Read-Host "Remove $UserDirectory (configuration or logs)? [y/N]") -match "^[yY]") {
            Remove-Item -Recurse -Force $UserDirectory
        }
    }
}

# This script is in the directory that is removed, so a separate process removes it afterwards.
Start-Process powershell.exe -WindowStyle Hidden -ArgumentList "-NoProfile", "-Command", "Start-Sleep 2; Remove-Item -Recurse -Force '$Directory'"
Write-Host "Removed the GameController from $Directory."
