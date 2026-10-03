[CmdletBinding()]
param([string]$DepotRoot = 'C:\MAIA\restricted-verifier-depot')
$ErrorActionPreference = 'Stop'
$tool = Join-Path $DepotRoot 'toolchains\stable-x86_64-pc-windows-msvc'
$vendor = Join-Path $DepotRoot 'vendor\m0165-r5v-generated-20260928'
$required = @(
    (Join-Path $tool 'bin\cargo.exe'),
    (Join-Path $tool 'bin\rustc.exe'),
    (Join-Path $tool 'bin\rustdoc.exe'),
    (Join-Path $tool 'bin\rustfmt.exe'),
    (Join-Path $DepotRoot 'msvc\bin\Hostx64\x64\cl.exe'),
    (Join-Path $DepotRoot 'msvc\bin\Hostx64\x64\link.exe'),
    (Join-Path $DepotRoot 'msvc\include\vcruntime.h'),
    (Join-Path $DepotRoot 'windows-sdk\include\ucrt\stdlib.h'),
    (Join-Path $DepotRoot 'windows-sdk\include\ucrt\stdio.h'),
    (Join-Path $DepotRoot 'windows-sdk\include\um\Windows.h'),
    (Join-Path $DepotRoot 'windows-sdk\include\shared\winapifamily.h'),
    (Join-Path $vendor 'ab_glyph\Cargo.toml'),
    (Join-Path $vendor 'ab_glyph\.cargo-checksum.json')
)
$missing = @($required | Where-Object { -not (Test-Path -LiteralPath $_ -PathType Leaf) })
if ($missing.Count) { throw ('Missing staged host resource(s): ' + ($missing -join '; ')) }
$packages = @(Get-ChildItem -LiteralPath $vendor -Directory)
$checksums = @(Get-ChildItem -LiteralPath $vendor -Filter '.cargo-checksum.json' -File -Recurse)
if ($packages.Count -ne 388 -or $checksums.Count -ne 388) { throw "Validated vendor shape changed: packages=$($packages.Count), checksums=$($checksums.Count); stop and revalidate the depot." }
$cargoHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $tool 'bin\cargo.exe')).Hash
if ($cargoHash -ne 'C37545EC61D48D31BDEFCE53280ECAB61C4EA54EAACE372FAC6A0316C6E165D9') { throw "Unexpected Cargo binary hash: $cargoHash" }
Write-Output 'HOST_RESOURCES=PASS (presence/count/hash only; see M0.16.5 R5V report for full archive validation evidence)'
Write-Output "CARGO_SHA256=$cargoHash"
Write-Output "VENDOR_PACKAGES=$($packages.Count); CHECKSUMS=$($checksums.Count)"
foreach ($path in $required) { Write-Output "PRESENT=$path" }