[CmdletBinding()]
param(
    [switch] $Remove,
    [switch] $ValidateOnly
)

$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'M0165NullStdinCapabilityAcl.cs'
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
    throw "Missing ACL helper source: $source"
}
Add-Type -Path $source

$mutex = [System.Threading.Mutex]::new($false, 'Local\MAIA-Evolution-Tier1-Verifier-v1')
$owned = $false
try {
    try { $owned = $mutex.WaitOne(0) } catch [System.Threading.AbandonedMutexException] { $owned = $true }
    if (-not $owned) { throw 'A Tier-1 verifier owns the host resource lock; retry after it exits.' }

    if ($ValidateOnly) {
        $result = [M0165NullStdinCapabilityAcl]::Probe()
        Write-Output $result
        if ($result -notmatch 'exact_read_only_noninherited_ace=True') {
            throw 'Required exact NUL capability ACE is not installed.'
        }
        return
    }

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Install/remove requires an elevated administrator PowerShell. ValidateOnly remains non-elevated.'
    }

    $result = [M0165NullStdinCapabilityAcl]::Apply([bool]$Remove)
    $action = if ($Remove) { 'remove' } else { 'install' }
    $recordRoot = 'C:\MAIA\reports\desk_outbox'
    New-Item -ItemType Directory -Path $recordRoot -Force | Out-Null
    $record = Join-Path $recordRoot ("m0165_null_stdin_capability_{0}_{1}.txt" -f $action, (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ'))
    $body = @(
        'resource=\Device\Null'
        'capability=maia.evolution.tier1.null.stdin'
        'access=FILE_GENERIC_READ (0x00120089); no inheritance'
        'network_capabilities=none added'
        ('operation=' + $action)
        ('utc=' + (Get-Date).ToUniversalTime().ToString('o'))
        $result
    ) -join "`r`n"
    [System.IO.File]::WriteAllText($record, $body + "`r`n", [System.Text.UTF8Encoding]::new($false))
    $hash = (Get-FileHash -LiteralPath $record -Algorithm SHA256).Hash
    Write-Output $result
    Write-Output "RESTORE_EVIDENCE=$record"
    Write-Output "RESTORE_EVIDENCE_SHA256=$hash"
}
finally {
    if ($owned) { [void]$mutex.ReleaseMutex() }
    $mutex.Dispose()
}
