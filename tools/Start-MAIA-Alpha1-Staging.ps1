#Requires -Version 5.1
<# Launch a built unsigned Alpha1 staging artifact with explicit local data.
No Cargo, model installation/start, service, connector or update is invoked.
The application retains its existing loopback probes and explicit briefing UI.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$DataDirectory,
    [string]$ArtifactDirectory = $PSScriptRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'Alpha1 staging artifacts require Windows.'
}
if ($DataDirectory -notmatch '^[A-Za-z]:[\\/]') {
    throw 'Choose an absolute local data directory for this staging profile.'
}
$data = [IO.Path]::GetFullPath($DataDirectory)
if ([IO.Path]::GetPathRoot($data) -notmatch '^[A-Za-z]:\\$') {
    throw 'The staging data directory must use a local drive path.'
}
if ($ArtifactDirectory -notmatch '^[A-Za-z]:[\\/]') {
    throw 'Choose an absolute local artifact directory.'
}
$artifact = [IO.Path]::GetFullPath($ArtifactDirectory)
if ([IO.Path]::GetPathRoot($artifact) -notmatch '^[A-Za-z]:\\$') {
    throw 'Choose an absolute local artifact directory.'
}
if (-not (Test-Path -LiteralPath $artifact -PathType Container)) {
    throw 'The literal artifact directory does not exist.'
}
$exe = Join-Path $artifact 'bin/maia-desktop.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw 'The artifact is missing bin/maia-desktop.exe.'
}
if (Test-Path -LiteralPath $data -PathType Leaf) {
    throw 'The selected data directory is a file.'
}
[void][IO.Directory]::CreateDirectory($data)
$info = New-Object Diagnostics.ProcessStartInfo
$info.FileName = $exe
$info.WorkingDirectory = $artifact
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.EnvironmentVariables['MAIA_ALPHA1_DATABASE'] = Join-Path $data 'workspace.sqlite3'
$info.EnvironmentVariables['MAIA_LOCAL_INTELLIGENCE_HEALTH_DB'] = Join-Path $data 'health.sqlite3'
$info.EnvironmentVariables['MAIA_ALPHA1_PREFERENCES'] = Join-Path $data 'ui.json'
$info.EnvironmentVariables['MAIA_ALPHA1_DIAGNOSTICS'] = Join-Path $data 'diagnostics.log'
# A normal packaged launch never inherits development auto-import/prompt/capture hooks.
foreach ($key in @($info.EnvironmentVariables.Keys)) {
    if ($key -like 'MAIA_REVIEW_*' -or $key -like 'MAIA_CAPTURE_*') {
        $info.EnvironmentVariables.Remove($key)
    }
}
$child = New-Object Diagnostics.Process
$child.StartInfo = $info
try {
    if (-not $child.Start()) { throw 'The staging application did not start.' }
    $child.WaitForExit()
    $launchExitCode = $child.ExitCode
}
finally { $child.Dispose() }
exit $launchExitCode
