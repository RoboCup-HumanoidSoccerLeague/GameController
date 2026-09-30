$ErrorActionPreference = "Stop"

$DestinationDirectory = $args[0]
if (!$DestinationDirectory) {
    throw "usage: $PSCommandPath <destdir> [<target>]"
}
$Target = $args[1]
if (!$Target) {
    $Target = $(rustc.exe -vV) -match "^host: (.*)" -replace "^host: ", ""
}
$BuildProfile = "release-dist"

$BaseDirectory = Split-Path $PSScriptRoot -Parent

Push-Location $BaseDirectory
cargo build --target $Target --profile $BuildProfile --package game_controller_api
Pop-Location

$IncludeDirectory = Join-Path $DestinationDirectory "include"
$LibraryDirectory = Join-Path $DestinationDirectory "lib"
$ConfigDirectory = Join-Path $DestinationDirectory "config"
New-Item -ItemType Directory -Force -Path $IncludeDirectory, $LibraryDirectory, $ConfigDirectory | Out-Null

$BuildDirectory = Join-Path $BaseDirectory "target\$Target\$BuildProfile"
Copy-Item $(Join-Path $BaseDirectory "LICENSE") $DestinationDirectory
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
