#Requires -Version 5.1
<#
Developer launch of the existing local Alpha1 briefing surface.
Cargo may fetch uncached dependencies. -Release selects optimization only;
it does not establish DEPLOYMENT_LOCKED or a signed release.
The application owns explicit evidence selection, SQLite and local-provider
behavior. This script does not install/start a model, service or connector.
#>
[CmdletBinding()]
param([switch]$Release)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'Alpha1 developer launch requires Windows.'
}
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$manifest = Join-Path $repository 'Cargo.toml'
foreach ($relative in @('Cargo.toml', 'apps/desktop/Cargo.toml', 'apps/desktop/src/main.rs')) {
    if (-not (Test-Path -LiteralPath (Join-Path $repository $relative) -PathType Leaf)) {
        throw "Expected MAIA checkout file is missing: $relative"
    }
}
$cargo = @(Get-Command cargo -CommandType Application -ErrorAction Stop)[0]
$cargoArguments = @('run', '--manifest-path', $manifest, '--locked',
    '--package', 'maia-desktop', '--bin', 'maia-desktop', '--no-default-features')
if ($Release) { $cargoArguments += '--release' }
Push-Location -LiteralPath $repository
try {
    & $cargo.Path @cargoArguments
    $launchExitCode = $LASTEXITCODE
}
finally { Pop-Location }
exit $launchExitCode
