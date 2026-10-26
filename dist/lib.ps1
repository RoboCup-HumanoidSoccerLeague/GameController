# Shared functions of the dist scripts. This file is dot-sourced, not executed.
#
# A product (app, api, logs) is built in two phases: Stage-<Product> creates the directory that is
# the content of the distribution, Pack-<Product> creates the archive from it.

$ErrorActionPreference = "Stop"

$BaseDirectory = Split-Path $PSScriptRoot -Parent
$DistDirectory = $PSScriptRoot
$BuildProfile = "release-dist"

# Sets $Target to the given target or the host.
function Initialize-Target($RequestedTarget) {
    if ($RequestedTarget) {
        $script:Target = $RequestedTarget
    } else {
        $script:Target = $(rustc.exe -vV) -match "^host: (.*)" -replace "^host: ", ""
    }
}

function Build-Frontend {
    Push-Location $(Join-Path $BaseDirectory "frontend")
    npm ci
    if ($LASTEXITCODE -ne 0) {
        throw "npm ci failed"
    }
    npm run build
    if ($LASTEXITCODE -ne 0) {
        throw "npm run build failed"
    }
    Pop-Location
}

# Builds the given cargo packages for $Target.
function Build-Packages([string[]]$Packages) {
    $PackageArguments = $Packages | ForEach-Object { "--package"; $_ }
    Push-Location $BaseDirectory
    cargo build --target $Target --profile $BuildProfile @PackageArguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed"
    }
    Pop-Location
}

function Get-BuildDirectory {
    Join-Path $BaseDirectory "target\$Target\$BuildProfile"
}

# Creates <directory>.zip from a directory.
function New-ZipArchive($Directory) {
    Compress-Archive $Directory "$Directory.zip" -Force
}

function Reset-Directory($Directory) {
    if (Test-Path $Directory) {
        Remove-Item -Recurse -Force $Directory
    }
    New-Item -ItemType Directory -Path $Directory | Out-Null
}

function Stage-Api {
    $script:ApiDirectory = Join-Path $DistDirectory "game_controller_api-$Version-$Target"
    Reset-Directory $ApiDirectory
    $IncludeDirectory = Join-Path $ApiDirectory "include"
    $LibraryDirectory = Join-Path $ApiDirectory "lib"
    $ConfigDirectory = Join-Path $ApiDirectory "config"
    New-Item -ItemType Directory -Force -Path $IncludeDirectory, $LibraryDirectory, $ConfigDirectory | Out-Null

    $BuildDirectory = Get-BuildDirectory
    Copy-Item $(Join-Path $BaseDirectory "LICENSE") $ApiDirectory
    Copy-Item $(Join-Path $BaseDirectory "game_controller_api\headers\GameController.h") $IncludeDirectory
    Copy-Item $(Join-Path $BaseDirectory "game_controller_msgs\headers\RoboCupGameControlData.h") $IncludeDirectory
    Copy-Item $(Join-Path $BuildDirectory "game_controller_api.dll") $LibraryDirectory
    Copy-Item $(Join-Path $BuildDirectory "game_controller_api.dll.lib") $LibraryDirectory
    foreach ($Competition in Get-ChildItem -Directory $(Join-Path $BaseDirectory "config")) {
        $Params = Join-Path $Competition.FullName "params.yaml"
        if (Test-Path $Params) {
            Copy-Item $Params $(Join-Path $ConfigDirectory "$($Competition.Name).yaml")
        }
    }
}

function Pack-Api {
    New-ZipArchive $ApiDirectory
}

function Stage-Logs {
    $script:LogsDirectory = Join-Path $DistDirectory "game_controller_logs-$Version-$Target"
    Reset-Directory $LogsDirectory
    Copy-Item $(Join-Path $BaseDirectory "LICENSE") $LogsDirectory
    Copy-Item $(Join-Path (Get-BuildDirectory) "game_controller_logs.exe") $LogsDirectory
}

function Pack-Logs {
    New-ZipArchive $LogsDirectory
}

# The archive can be run in place, or installed for the current user with install.ps1.
function Stage-App {
    $script:AppDirectory = Join-Path $DistDirectory "GameController-$Version-$Target"
    Reset-Directory $AppDirectory
    Copy-Item $(Join-Path $BaseDirectory "LICENSE") $AppDirectory
    Copy-Item $(Join-Path $BaseDirectory "README.md") $AppDirectory
    Copy-Item $(Join-Path $BaseDirectory "config") $AppDirectory -Recurse
    Copy-Item $(Join-Path (Get-BuildDirectory) "game_controller_app.exe") $(Join-Path $AppDirectory "GameController.exe")
    Set-Content -NoNewline -Path $(Join-Path $AppDirectory "version.txt") -Value $Version
    Copy-Item $(Join-Path $DistDirectory "windows\install.ps1") $AppDirectory
    Copy-Item $(Join-Path $DistDirectory "windows\uninstall.ps1") $AppDirectory
}

function Pack-App {
    New-ZipArchive $AppDirectory
}
