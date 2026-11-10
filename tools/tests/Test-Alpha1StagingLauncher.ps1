#Requires -Version 5.1
<# Finite Windows PowerShell5.1 fake-EXE checks, never a MAIA/model launch. #>
[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$launcher = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../Start-MAIA-Alpha1-Staging.ps1'))
$powershell = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::System)) 'WindowsPowerShell/v1.0/powershell.exe'
$root = Join-Path ([IO.Path]::GetTempPath()) ('maia staging fixture ' + [guid]::NewGuid().ToString('N'))
$artifact = Join-Path $root 'artifact with spaces'
$data = Join-Path $root 'profile with spaces'
$exe = Join-Path $artifact 'bin/maia-desktop.exe'
$success = $false
$keys = @('MAIA_ALPHA1_DATABASE', 'MAIA_LOCAL_INTELLIGENCE_HEALTH_DB', 'MAIA_ALPHA1_PREFERENCES', 'MAIA_ALPHA1_DIAGNOSTICS', 'MAIA_CAPTURE_PATH', 'MAIA_REVIEW_PROMPT')
$before = @{}
foreach ($key in $keys) { $before[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
function Need([bool]$Value, [string]$Message) { if (-not $Value) { throw $Message } }
function Quote([string]$Value) {
    Need (-not $Value.Contains('"')) 'Synthetic arguments must not contain quotes.'
    return '"' + $Value + '"'
}
function Run-Launcher([string]$SelectedData, [string]$ArtifactPath, [string]$Extra = '') {
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $powershell
    $info.Arguments = '-NoProfile -NonInteractive -File ' + (Quote $launcher) + ' -ArtifactDirectory ' + (Quote $ArtifactPath) + ' -DataDirectory ' + (Quote $SelectedData) + ' ' + $Extra
    $info.WorkingDirectory = $root
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($key in $keys) { $info.EnvironmentVariables[$key] = 'synthetic inherited value' }
    $child = New-Object Diagnostics.Process
    $child.StartInfo = $info
    $started = $false
    try {
        $started = $child.Start()
        Need $started 'Synthetic launcher child did not start.'
        $handle = $child.Handle
        $stdout = $child.StandardOutput.ReadToEndAsync()
        $stderr = $child.StandardError.ReadToEndAsync()
        Need ($child.WaitForExit(20000)) 'Synthetic launcher child exceeded20seconds.'
        $code = $child.ExitCode
        $out = $stdout.GetAwaiter().GetResult()
        $err = $stderr.GetAwaiter().GetResult()
        return @{ exit_code = $code; stdout = $out; stderr = $err }
    }
    finally {
        if ($started -and -not $child.HasExited) { $child.Kill(); [void]$child.WaitForExit(5000) }
        $child.Dispose()
    }
}
try {
    [void][IO.Directory]::CreateDirectory((Split-Path -Parent $exe))
    $workingMarker = [guid]::NewGuid().ToString('N')
    [IO.File]::WriteAllText((Join-Path $artifact 'working-directory-marker.txt'), $workingMarker)
    $fake = @'
using System;
using System.IO;
public static class FixtureAlphaProgram {
    public static int Main() {
        string[] names = { "MAIA_ALPHA1_DATABASE", "MAIA_LOCAL_INTELLIGENCE_HEALTH_DB", "MAIA_ALPHA1_PREFERENCES", "MAIA_ALPHA1_DIAGNOSTICS", "MAIA_CAPTURE_PATH", "MAIA_REVIEW_PROMPT" };
        string folder = Path.GetDirectoryName(Environment.GetEnvironmentVariable(names[0]));
        string[] observed = new string[8];
        for (int i = 0; i < names.Length; i++) observed[i] = Environment.GetEnvironmentVariable(names[i]) ?? "<absent>";
        observed[6] = Environment.CurrentDirectory;
        observed[7] = File.ReadAllText("working-directory-marker.txt");
        File.WriteAllLines(Path.Combine(folder, "observed.txt"), observed);
        return 23;
    }
}
'@
    $fake = $fake.Replace('FixtureAlphaProgram', ('FixtureAlphaProgram' + [guid]::NewGuid().ToString('N')))
    Add-Type -TypeDefinition $fake -OutputAssembly $exe -OutputType ConsoleApplication
    $result = Run-Launcher $data $artifact
    Need ($result.exit_code -eq 23) ('Expected fake exit23; got ' + $result.exit_code + ': ' + $result.stderr)
    $observed = @(Get-Content -LiteralPath (Join-Path $data 'observed.txt'))
    Need ($observed.Count -eq 8) 'Fake EXE did not record its exact environment.'
    $expected = @((Join-Path $data 'workspace.sqlite3'), (Join-Path $data 'health.sqlite3'), (Join-Path $data 'ui.json'), (Join-Path $data 'diagnostics.log'), '<absent>', '<absent>')
    for ($i = 0; $i -lt 6; $i++) { Need ($observed[$i] -ceq $expected[$i]) ('Child profile/hook/working directory mismatch at ' + $i) }
    Need ($observed[7] -ceq $workingMarker) 'Fake EXE did not read the unique marker from the artifact working directory.'
    $missingData = Join-Path $root 'must not create missing-exe data'
    [void][IO.Directory]::CreateDirectory((Join-Path $root 'missing artifact'))
    $result = Run-Launcher $missingData (Join-Path $root 'missing artifact')
    Need ($result.exit_code -ne 0 -and $result.stderr.Contains('missing bin/maia-desktop.exe') -and -not (Test-Path -LiteralPath $missingData)) 'Missing EXE must fail for its specific binary reason before creating data.'
    $unknownData = Join-Path $root 'must not create unknown-option data'
    $result = Run-Launcher $unknownData $artifact '-UnknownOption'
    Need ($result.exit_code -ne 0 -and -not (Test-Path -LiteralPath $unknownData)) 'Unknown flag must fail before launching.'
    $driveRelative = ([IO.Path]::GetPathRoot($data)).Substring(0, 2) + 'relative-profile'
    $result = Run-Launcher $driveRelative $artifact
    Need ($result.exit_code -ne 0) 'Drive-relative data must be refused.'
    $rootRelative = $data.Substring(2)
    $result = Run-Launcher $rootRelative $artifact
    Need ($result.exit_code -ne 0) 'Root-relative data must be refused.'
    $artifactDrive = ([IO.Path]::GetPathRoot($artifact)).Substring(0, 2)
    foreach ($badArtifact in @('artifact with spaces', ($artifactDrive + 'artifact with spaces'))) {
        $refusedData = Join-Path $root ('refused artifact profile ' + [guid]::NewGuid().ToString('N'))
        $result = Run-Launcher $refusedData $badArtifact
        Need ($result.exit_code -ne 0 -and $result.stderr.Contains('absolute local artifact directory')) 'Relative/drive-relative artifact must fail for its explicit absolute-path contract.'
        Need (-not (Test-Path -LiteralPath $refusedData) -and -not (Test-Path -LiteralPath (Join-Path $refusedData 'observed.txt'))) 'Refused artifact path must not create data or a fake-EXE marker.'
    }
    foreach ($key in $keys) { Need ([Environment]::GetEnvironmentVariable($key, 'Process') -ceq $before[$key]) ('Parent environment changed: ' + $key) }
    $success = $true
    Write-Output 'PASS: six finite fake-EXE staging checks; no product/model execution qualification.'
}
finally {
    if ($success) { Remove-Item -LiteralPath $root -Recurse -Force }
    else { Write-Output ('Retained synthetic fixture: ' + $root) }
}
