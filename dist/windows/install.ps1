# This script installs the GameController from this directory (an extracted archive) for the
# current user. It adds a Start menu shortcut and an entry in "Apps & Features". Installing over an
# existing installation updates it. The configuration and logs of the user are not touched.

param(
    [string]$Directory = $(Join-Path $env:LOCALAPPDATA "Programs\GameController"),
    # Adds an inbound firewall rule for the program (needs an elevated PowerShell).
    [switch]$AddFirewallRule
)

$ErrorActionPreference = "Stop"

$Source = $PSScriptRoot
$Version = (Get-Content -Raw (Join-Path $Source "version.txt")).Trim()
$Registry = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\GameController"
$StartMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$Shortcut = Join-Path $StartMenu "GameController.lnk"

if (Test-Path $Directory) {
    Remove-Item -Recurse -Force $Directory
}
New-Item -ItemType Directory -Path $Directory | Out-Null
foreach ($Item in "GameController.exe", "config", "LICENSE", "README.md", "version.txt", "uninstall.ps1") {
    Copy-Item (Join-Path $Source $Item) $Directory -Recurse
}

$Executable = Join-Path $Directory "GameController.exe"
$Link = (New-Object -ComObject WScript.Shell).CreateShortcut($Shortcut)
$Link.TargetPath = $Executable
$Link.WorkingDirectory = $Directory
$Link.Save()

$Uninstall = Join-Path $Directory "uninstall.ps1"
New-Item -Force -Path $Registry | Out-Null
Set-ItemProperty $Registry DisplayName "GameController"
Set-ItemProperty $Registry DisplayVersion $Version
Set-ItemProperty $Registry Publisher "RoboCup Humanoid Soccer League"
Set-ItemProperty $Registry InstallLocation $Directory
Set-ItemProperty $Registry DisplayIcon $Executable
Set-ItemProperty $Registry UninstallString "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$Uninstall`""
Set-ItemProperty $Registry NoModify 1 -Type DWord
Set-ItemProperty $Registry NoRepair 1 -Type DWord

if ($AddFirewallRule) {
    New-NetFirewallRule -DisplayName "GameController" -Direction Inbound -Program $Executable -Action Allow | Out-Null
}

Write-Host "Installed the GameController to $Directory."
