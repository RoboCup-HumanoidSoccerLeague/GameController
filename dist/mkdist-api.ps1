$ErrorActionPreference = "Stop"

$Version = $args[0] -replace "^v", ""
if (!$Version) {
    throw "usage: $PSCommandPath <version> [<target>]"
}
$Target = $args[1]
if (!$Target) {
    $Target = $(rustc.exe -vV) -match "^host: (.*)" -replace "^host: ", ""
}

$ArchiveDirectory = Join-Path $PSScriptRoot "game_controller_api-$Version-$Target"
$Archive = Join-Path $PSScriptRoot "game_controller_api-$Version-$Target.zip"

if (Test-Path $ArchiveDirectory) {
    Remove-Item -Recurse -Force $ArchiveDirectory
}

& $(Join-Path $PSScriptRoot "install-api.ps1") $ArchiveDirectory $Target

Compress-Archive $ArchiveDirectory $Archive -Force
