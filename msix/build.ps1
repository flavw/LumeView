param([string]$SdkBin)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot
if(!$SdkBin){$SdkBin=Join-Path $root '.dev/msix-sdk/bin/10.0.28000.0/x64'}
$makeappx=Join-Path $SdkBin 'makeappx.exe'
$staging=Join-Path $PSScriptRoot 'staging'
$releaseRoot=if($env:CARGO_TARGET_DIR){Join-Path $env:CARGO_TARGET_DIR 'release'}else{Join-Path $root 'target/release'}
New-Item -ItemType Directory -Force $staging,(Join-Path $root 'build') | Out-Null
Copy-Item -LiteralPath (Join-Path $releaseRoot 'lumeview.exe') -Destination $staging -Force
foreach($name in @('tools','LICENSE','PRIVACY.md','THIRD_PARTY_LICENSES.txt')){Copy-Item -LiteralPath (Join-Path $root $name) -Destination $staging -Recurse -Force}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Assets'),(Join-Path $PSScriptRoot 'AppxManifest.xml') -Destination $staging -Recurse -Force
[xml]$manifest=Get-Content (Join-Path $staging 'AppxManifest.xml') -Raw
$output=Join-Path $root ('build/LumeView_'+$manifest.Package.Identity.Version+'_x64_Store.msix')
& $makeappx pack /d $staging /p $output /o
if($LASTEXITCODE -ne 0){throw 'MSIX build failed'}
