# Builds distributions of the GameController into dist/. The products are the application (app),
# the C API (api) and the log analysis tool (logs); by default all are built, in one cargo build.

param(
    [string[]]$Only = @("app", "api", "logs"),
    [switch]$StageOnly,
    [Parameter(Mandatory = $true, Position = 0)][string]$Version,
    [Parameter(Position = 1)][string]$Target
)

. $(Join-Path $PSScriptRoot "lib.ps1")

$Version = $Version -replace "^v", ""
Initialize-Target $Target

$Packages = @()
foreach ($Product in $Only) {
    switch ($Product) {
        "app" { $Packages += "game_controller_app" }
        "api" { $Packages += "game_controller_api" }
        "logs" { $Packages += "game_controller_logs" }
        default { throw "unknown product $Product" }
    }
}

if ($Only -contains "app") {
    Build-Frontend
}
Build-Packages $Packages

foreach ($Product in $Only) {
    & "Stage-$Product"
    if (!$StageOnly) {
        & "Pack-$Product"
    }
}
