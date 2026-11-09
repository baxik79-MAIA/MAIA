#Requires -Version 5.1
# Runs synthetic Cargo command files only. It never builds or starts the product/model.
[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'These launcher process checks require Windows.'
}
$launcherSource = Join-Path (Split-Path -Parent $PSScriptRoot) 'Start-MAIA-Alpha1.ps1'
$root = Join-Path ([IO.Path]::GetTempPath()) ('maia-launcher-check-' + [Guid]::NewGuid().ToString('N'))
$repository = Join-Path $root 'checkout with spaces'
$bin = Join-Path $root 'fake bin'
$laterBin = Join-Path $root 'later fake bin'
$foreign = Join-Path $root 'unrelated cwd'
$savedPath = $env:PATH
$savedLog = $env:MAIA_LAUNCHER_TEST_LOG
$savedCwd = $env:MAIA_LAUNCHER_TEST_CWD
$savedLaterLog = $env:MAIA_LAUNCHER_TEST_LATER_LOG
$passed = $false
try {
    foreach ($folder in @($repository, $bin, $laterBin, $foreign, (Join-Path $repository 'tools'), (Join-Path $repository 'apps/desktop/src'))) {
        [void](New-Item -ItemType Directory -Path $folder -Force)
    }
    foreach ($relative in @('Cargo.toml', 'apps/desktop/Cargo.toml', 'apps/desktop/src/main.rs')) {
        [IO.File]::WriteAllText((Join-Path $repository $relative), 'synthetic launcher fixture')
    }
    $launcher = Join-Path $repository 'tools/Start-MAIA-Alpha1.ps1'
    Copy-Item -LiteralPath $launcherSource -Destination $launcher
    [IO.File]::WriteAllText((Join-Path $bin 'cargo.cmd'), "@echo off`r`n>>`"%MAIA_LAUNCHER_TEST_LOG%`" echo %*`r`n>`"%MAIA_LAUNCHER_TEST_CWD%`" cd`r`nexit /b 23`r`n")
    [IO.File]::WriteAllText((Join-Path $laterBin 'cargo.bat'), "@echo off`r`n>>`"%MAIA_LAUNCHER_TEST_LATER_LOG%`" echo %*`r`nexit /b 97`r`n")
    $env:PATH = "$bin;$laterBin;$savedPath"
    $applications = @(Get-Command cargo -CommandType Application -ErrorAction Stop)
    [IO.File]::WriteAllText((Join-Path $root 'cargo.discovery.json'), ($applications.Path | ConvertTo-Json -Compress))
    if ($applications.Count -lt 2 -or $applications[0].Path -ne (Join-Path $bin 'cargo.cmd') -or -not ($applications.Path -contains (Join-Path $laterBin 'cargo.bat'))) {
        throw "Synthetic fixture did not expose ordered Cargo applications; fixture=$root"
    }
    function Invoke-Case([string]$Name, [string[]]$Extra, [bool]$ExpectedCargo) {
        $env:MAIA_LAUNCHER_TEST_LOG = Join-Path $root "$Name.args"
        $env:MAIA_LAUNCHER_TEST_CWD = Join-Path $root "$Name.cwd"
        $env:MAIA_LAUNCHER_TEST_LATER_LOG = Join-Path $root "$Name.later.args"
        $stdout = Join-Path $root "$Name.stdout"
        $stderr = Join-Path $root "$Name.stderr"
        $arguments = @('-NoLogo', '-NoProfile', '-NonInteractive', '-File', ('"' + $launcher + '"')) + $Extra
        $process = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList $arguments -WorkingDirectory $foreign -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru -NoNewWindow
        $exited = $false
        $timedOut = $false
        $actualExitCode = $null
        try {
            [void]$process.Handle
            $exited = $process.WaitForExit(15000)
            if (-not $exited) {
                $timedOut = $true
                $process.Kill(); $exited = $process.WaitForExit(5000)
                throw "$Name exceeded its finite fake-tool deadline; fixture retained at $root."
            }
            $actualExitCode = $process.ExitCode
            $exitDescription = if ($null -eq $actualExitCode) { '<null/unobserved>' } else { "$actualExitCode ($($actualExitCode.GetType().FullName))" }
            $stderrText = if (Test-Path -LiteralPath $stderr) { Get-Content -LiteralPath $stderr -Raw } else { '<stderr file missing>' }
            if ($null -eq $actualExitCode) { throw "$Name has no observed exit code; stderr=$stderrText; fixture=$root" }
            if ($ExpectedCargo) {
                if ($actualExitCode -ne 23) { throw "$Name lost the cargo exit code: expected23, observed=$exitDescription; stderr=$stderrText; fixture=$root" }
                $lines = @(Get-Content -LiteralPath $env:MAIA_LAUNCHER_TEST_LOG)
                if ($lines.Count -ne 1) { throw "$Name invoked cargo more than once." }
                $text = $lines[0]
                if (-not $text.StartsWith('run ')) { throw "$Name did not select the run command." }
                foreach ($required in @('--locked', '--package maia-desktop', '--bin maia-desktop', '--no-default-features', (Join-Path $repository 'Cargo.toml'))) {
                    if (-not $text.Contains($required)) { throw "$Name lost a reviewed argument: $required" }
                }
                if ((Get-Content -LiteralPath $env:MAIA_LAUNCHER_TEST_CWD -Raw).Trim() -ne $repository) { throw "$Name used the caller working directory." }
                if ($text.Contains('--release') -ne ($Extra -contains '-Release')) { throw "$Name changed build configuration." }
            }
            elseif ($actualExitCode -eq 0 -or (Test-Path -LiteralPath $env:MAIA_LAUNCHER_TEST_LOG)) {
                throw "$Name did not refuse before tool invocation: observed=$exitDescription; stderr=$stderrText; fixture=$root"
            }
            if (Test-Path -LiteralPath $env:MAIA_LAUNCHER_TEST_LATER_LOG) { throw "$Name selected a later Cargo application." }
        }
        finally {
            try {
                $exitType = if ($null -eq $actualExitCode) { 'null/unobserved' } else { $actualExitCode.GetType().FullName }
                $caseEvidence = [ordered]@{ case=$Name; pid=$process.Id; wait_completed=$exited; exit_observed=($exited -and $null -ne $actualExitCode); exit_code=$actualExitCode; exit_type=$exitType; timed_out=$timedOut; expected_cargo=$ExpectedCargo; stdout_path=$stdout; stderr_path=$stderr; first_cargo_log=$env:MAIA_LAUNCHER_TEST_LOG; later_cargo_log=$env:MAIA_LAUNCHER_TEST_LATER_LOG }
                [IO.File]::WriteAllText((Join-Path $root "$Name.result.json"), ($caseEvidence | ConvertTo-Json -Compress))
            }
            finally { $process.Dispose() }
        }
    }
    Invoke-Case -Name 'debug-spaces-exit' -Extra @() -ExpectedCargo $true
    Invoke-Case -Name 'release-spaces-exit' -Extra @('-Release') -ExpectedCargo $true
    Invoke-Case -Name 'unknown-feature-refused' -Extra @('-Features', 'development-evolution') -ExpectedCargo $false
    Remove-Item -LiteralPath (Join-Path $repository 'Cargo.toml')
    Invoke-Case -Name 'missing-checkout-refused' -Extra @() -ExpectedCargo $false
    $passed = $true
    Write-Output 'PASS: four finite fake-cargo cases; no product build, model or execution qualification.'
}
finally {
    $env:PATH = $savedPath
    $env:MAIA_LAUNCHER_TEST_LOG = $savedLog
    $env:MAIA_LAUNCHER_TEST_CWD = $savedCwd
    $env:MAIA_LAUNCHER_TEST_LATER_LOG = $savedLaterLog
    if ($passed) {
        if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
    }
    else { [Console]::Error.WriteLine("Synthetic launcher fixture retained: $root") }
}
