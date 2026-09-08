param([ValidateSet('x64','arm64')][string]$Arch = 'x64', [string]$Language = 'ru')
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$repo = Split-Path $PSScriptRoot -Parent
$results = Join-Path $repo ".build/cross/smoke-windows-$Arch"
$check = Join-Path $env:TEMP ("hui-smoke-$Arch-" + [DateTime]::Now.Ticks)
New-Item -ItemType Directory -Force $results | Out-Null
Expand-Archive -Path "$repo/dist/HUI-windows-$Arch.zip" -DestinationPath $check
$app = "$check/HUI-windows-$Arch/hui.exe"
Copy-Item "$repo/packaging/smoke/multilingual.md" "$check/desktop.md"
$oldPath = $env:PATH
$oldData = $env:HUI_DATA_DIR
$oldBackend = $env:SLINT_BACKEND
$oldLanguage = $env:HUI_LANGUAGE
try {
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
    $env:HUI_DATA_DIR = "$check/profile"
    $env:SLINT_BACKEND = $null
    $env:HUI_LANGUAGE = $Language
    foreach ($format in @('png','pdf')) {
        & $app --render "$check/desktop.md" "$check/document.$format"
        if ($LASTEXITCODE -ne 0) { throw "Rendering failed: $Arch $format ($LASTEXITCODE)" }
        Copy-Item "$check/document.$format" $results -Force
    }
    & $app --screenshot "$check/window.png" split
    if ($LASTEXITCODE -ne 0) { throw "Window startup failed: $Arch ($LASTEXITCODE)" }
    Copy-Item "$check/window.png" $results -Force
    $live = Start-Process -FilePath $app -ArgumentList ('"{0}"' -f "$check/desktop.md") -PassThru -RedirectStandardError "$check/startup.log"
    try {
        Start-Sleep -Seconds 5
        $live.Refresh()
        if ($live.HasExited) { throw "Default renderer exited early: $Arch" }
    } finally {
        if (-not $live.HasExited) { Stop-Process -Id $live.Id }
        Copy-Item "$check/startup.log" $results -Force
    }
    Write-Host "PASS Windows ${Arch}: archive, PNG/PDF rendering, window and default renderer startup"
} finally {
    $env:PATH = $oldPath
    $env:HUI_DATA_DIR = $oldData
    $env:SLINT_BACKEND = $oldBackend
    $env:HUI_LANGUAGE = $oldLanguage
}
