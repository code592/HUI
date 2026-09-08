param([ValidateSet('x64','arm64')][string]$Arch = 'x64')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$python = if ($env:PYTHON3) { $env:PYTHON3 } else { (Get-Command python.exe -ErrorAction Stop).Source }
& $python --version
if ($LASTEXITCODE -ne 0) { throw 'Python 3 is required by the native CSS layout engine.' }
$env:PYTHON3 = $python
$target = if ($Arch -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
& cargo build --release -p hui-app --locked --target $target -j 2
if ($LASTEXITCODE -ne 0) { throw "Build failed: $target" }
$bundle = Join-Path $PWD "dist/HUI-windows-$Arch"
New-Item -ItemType Directory -Force "$bundle/assets/katex", "$bundle/licenses" | Out-Null
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
Copy-Item -Force "$targetDir/$target/release/hui.exe" $bundle
Copy-Item -Force packaging/windows/hui.ico $bundle
Copy-Item -Force vendor/katex/katex.min.css,vendor/katex/LICENSE "$bundle/assets/katex"
Copy-Item -Force vendor/katex/fonts "$bundle/assets/katex" -Recurse
Copy-Item -Force LICENSE "$bundle/licenses/HUI-LICENSE"
Copy-Item -Force vendor/mathjax/LICENSE "$bundle/licenses/MathJax-LICENSE"
Copy-Item -Force vendor/slint/LICENSE.md "$bundle/licenses/Slint-LICENSE.md"
Copy-Item -Force THIRD_PARTY.md "$bundle/licenses/THIRD_PARTY.md"
# App-local MSVC runtime makes the portable archive usable on clean machines.
if (-not $env:VCToolsRedistDir) { throw 'Run from a Visual Studio developer environment (VCToolsRedistDir required).' }
$crt = Get-ChildItem "$env:VCToolsRedistDir/$Arch" -Directory -Filter 'Microsoft.VC*.CRT' | Select-Object -First 1
if (-not $crt) { throw "Missing MSVC runtime for $Arch" }
Copy-Item -Force "$($crt.FullName)/*.dll" $bundle
"HUI Windows $Arch`r`nRun hui.exe or pass a .md file. Portable package; use Windows Open With to associate Markdown files." | Set-Content "$bundle/README.txt"
$zip = Join-Path $PWD "dist/HUI-windows-$Arch.zip"
Compress-Archive -Path $bundle -DestinationPath $zip -Force
(Get-FileHash $zip -Algorithm SHA256).Hash.ToLower() + '  ' + (Split-Path $zip -Leaf) | Set-Content "$zip.sha256"
Write-Host "Built $zip"
