[CmdletBinding()]
param([string]$OutputPath = (Join-Path $env:TEMP 'LadderR5V15m.exe'))
$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'LadderR5V.cs'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$destination = [IO.Path]::GetFullPath($OutputPath)
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Launcher source missing: $source" }
if (-not (Test-Path -LiteralPath $compiler -PathType Leaf)) { throw "Windows .NET Framework C# compiler missing: $compiler" }
if (Test-Path -LiteralPath $destination) { throw "Refusing to overwrite existing output: $destination" }
$parent = Split-Path -Parent $destination
New-Item -ItemType Directory -Force -Path $parent | Out-Null
& $compiler /nologo /target:exe "/out:$destination" $source
if ($LASTEXITCODE -ne 0) { throw "Launcher compile failed; csc exit=$LASTEXITCODE." }
Write-Output "LAUNCHER=$destination"
Write-Output "SHA256=$((Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash)"