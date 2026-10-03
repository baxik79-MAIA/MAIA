[CmdletBinding(DefaultParameterSetName = 'Profile')]
param(
    [Parameter(Mandatory, ParameterSetName = 'Profile')][string] $ProfileName,
    # ACE-only mode for a per-run package SID whose profile mapping no longer
    # exists. It removes only explicit ACEs for that SID and deletes nothing else.
    [Parameter(Mandatory, ParameterSetName = 'DeletedProfileSid')][string] $DeletedProfileSid,
    [Parameter(Mandatory)][string[]] $ScanRoots,
    [string[]] $ExactPaths = @(),
    [Parameter(Mandatory)][string] $ReportDirectory,
    [switch] $Apply
)

$ErrorActionPreference = 'Stop'
$aceOnly = $PSCmdlet.ParameterSetName -eq 'DeletedProfileSid'
$ownerPid = $null
if (-not $aceOnly) {
    if ($ProfileName -match '^maia-tier1-(\d+)-[0-9a-f]+-[0-9a-f]+$') {
        $ownerPid = [int]$Matches[1]
        if (Get-Process -Id $ownerPid -ErrorAction SilentlyContinue) {
            throw "Profile owner PID $ownerPid is still active; stop and investigate."
        }
    } elseif ($ProfileName -notmatch '^maia\.m0165\.probe\.[0-9a-f]{32}$') {
        # The R5V scratch launcher (LadderR5V.cs) names its single probe profile
        # MAIA.M0165.Probe.<guid>; it records no owner PID in the name.
        throw 'Expected one MAIA Tier-1 or R5V probe per-run AppContainer profile name.'
    }
}
if ($Apply) {
    $builders = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
        $_.ProcessName -match '^(cargo|rustc|rustdoc|clippy-driver|cargo-clippy|rustfmt|link)$'
    })
    if ($builders.Count) { throw 'A compiler or verifier descendant may still be active; stop and investigate.' }
}
$mappingRoot = 'Registry::HKEY_CURRENT_USER\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppContainer\Mappings'
if ($aceOnly) {
    $sid = $DeletedProfileSid
    if (Test-Path -LiteralPath (Join-Path $mappingRoot $sid)) {
        throw 'The package SID still has a profile mapping; use -ProfileName instead.'
    }
    $ProfileName = $null
    $mappings = @()
} else {
    $mappings = @(Get-ChildItem -LiteralPath $mappingRoot | Where-Object {
        (Get-ItemProperty -LiteralPath $_.PSPath).DisplayName -eq $ProfileName
    })
    if ($mappings.Count -ne 1) { throw "Expected exactly one matching AppContainer mapping; found $($mappings.Count)." }
    $sid = $mappings[0].PSChildName
}
if ($sid -notmatch '^S-1-15-2-(\d+-){6}\d+$') { throw "Unexpected package SID: $sid" }

$paths = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$scans = @()
foreach ($root in $ScanRoots) {
    $resolved = (Resolve-Path -LiteralPath $root -ErrorAction Stop).ProviderPath
    $output = @(& icacls.exe $resolved /findsid "*$sid" /T /C 2>&1)
    if ($LASTEXITCODE -ne 0 -or ($output | Select-String 'Failed processing [1-9]')) {
        throw "SID scan failed for $resolved; no ACL was changed."
    }
    foreach ($line in $output) {
        if ($line -match '^SID Found: (.*)\.$') { [void]$paths.Add($Matches[1]) }
    }
    $scans += [pscustomobject]@{ root = $resolved; hits = @($output | Where-Object { $_ -like 'SID Found:*' }).Count }
}
foreach ($path in $ExactPaths) {
    [void]$paths.Add((Resolve-Path -LiteralPath $path -ErrorAction Stop).ProviderPath)
}
function Get-ExplicitRules($acl) {
    @($acl.Access | Where-Object { -not $_.IsInherited } |
        ForEach-Object { "$($_.IdentityReference.Value)|$($_.AccessControlType)|$($_.FileSystemRights)|$($_.InheritanceFlags)|$($_.PropagationFlags)" } | Sort-Object)
}
$explicit = @()
foreach ($path in ($paths | Sort-Object)) {
    $acl = Get-Acl -LiteralPath $path
    $rules = @($acl.Access | Where-Object { $_.IdentityReference.Value -eq $sid -and -not $_.IsInherited })
    if ($rules.Count) {
        $explicit += [pscustomobject]@{
            path = $path
            sddl_before = $acl.Sddl
            explicit_rules_before = Get-ExplicitRules $acl
            rule_count = $rules.Count
            unrelated_rules = @($acl.Access | Where-Object { $_.IdentityReference.Value -ne $sid } |
                ForEach-Object { "$($_.IdentityReference.Value)|$($_.AccessControlType)|$($_.FileSystemRights)|$($_.IsInherited)|$($_.InheritanceFlags)|$($_.PropagationFlags)" } | Sort-Object)
        }
    }
}
New-Item -ItemType Directory -Path $ReportDirectory -Force | Out-Null
$reportRoot = (Resolve-Path -LiteralPath $ReportDirectory).ProviderPath
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffffffZ')
$before = Join-Path $reportRoot "orphan-$stamp-before.json"
[pscustomobject]@{
    profile = $ProfileName; sid = $sid; owner_pid = $ownerPid
    scan_roots = $scans; exact_paths = $ExactPaths; explicit_aces = $explicit
    action = if ($Apply) { 'apply' } else { 'inspect' }
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $before -Encoding utf8
Write-Output "EVIDENCE_BEFORE=$before"
Write-Output "EXPLICIT_ACE_PATHS=$($explicit.Count)"
if (-not $Apply) { return }

foreach ($entry in $explicit) {
    $current = Get-Acl -LiteralPath $entry.path
    # Removing this SID from a parent legitimately rewrites inherited entries
    # on a nested path, so drift is judged on explicit (non-inherited) rules.
    if (Compare-Object $entry.explicit_rules_before (Get-ExplicitRules $current)) {
        throw "Explicit ACL drift before cleanup: $($entry.path)"
    }
    $output = @(& icacls.exe $entry.path /remove:g "*$sid" 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "Exact SID removal failed: $($entry.path): $output" }
    $after = Get-Acl -LiteralPath $entry.path
    if (@($after.Access | Where-Object { $_.IdentityReference.Value -eq $sid -and -not $_.IsInherited }).Count) {
        throw "Explicit package SID ACE remains: $($entry.path)"
    }
    $otherAfter = @($after.Access | Where-Object { $_.IdentityReference.Value -ne $sid } |
        ForEach-Object { "$($_.IdentityReference.Value)|$($_.AccessControlType)|$($_.FileSystemRights)|$($_.IsInherited)|$($_.InheritanceFlags)|$($_.PropagationFlags)" } | Sort-Object)
    if (Compare-Object $entry.unrelated_rules $otherAfter) {
        throw "Unrelated ACL rule changed: $($entry.path)"
    }
}
foreach ($root in $ScanRoots) {
    $output = @(& icacls.exe $root /findsid "*$sid" /T /C 2>&1)
    if ($LASTEXITCODE -ne 0 -or @($output | Where-Object { $_ -like 'SID Found:*' }).Count) {
        throw "Package SID remains or rescan failed under $root; profile retained."
    }
}
foreach ($path in $ExactPaths) {
    if (@((Get-Acl -LiteralPath $path).Access | Where-Object { $_.IdentityReference.Value -eq $sid }).Count) {
        throw "Package SID remains on $path; profile retained."
    }
}
if ($aceOnly) {
    $afterRecord = Join-Path $reportRoot "orphan-$stamp-after.json"
    [pscustomobject]@{
        sid = $sid; mode = 'deleted_profile_ace_only'; removed_explicit_ace_paths = $explicit.Count
        completed_utc = (Get-Date).ToUniversalTime().ToString('o')
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $afterRecord -Encoding utf8
    Write-Output "EVIDENCE_AFTER=$afterRecord"
    Write-Output 'ORPHAN_ACE_CLEANUP=VERIFIED'
    return
}
$source = 'using System; using System.Runtime.InteropServices; public static class MaiaOrphanProfile { [DllImport("userenv.dll", CharSet=CharSet.Unicode)] public static extern int DeleteAppContainerProfile(string name); }'
Add-Type -TypeDefinition $source
$hr = [MaiaOrphanProfile]::DeleteAppContainerProfile($ProfileName)
$mappingRemains = Test-Path -LiteralPath $mappings[0].PSPath
$afterRecord = Join-Path $reportRoot "orphan-$stamp-after.json"
[pscustomobject]@{
    profile = $ProfileName; sid = $sid; removed_explicit_ace_paths = $explicit.Count
    delete_profile_hresult = $hr; mapping_remains = $mappingRemains
    completed_utc = (Get-Date).ToUniversalTime().ToString('o')
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $afterRecord -Encoding utf8
Write-Output "EVIDENCE_AFTER=$afterRecord"
if ($hr -ne 0 -or $mappingRemains) { throw 'Profile deletion was not verified.' }
Write-Output 'ORPHAN_CLEANUP=VERIFIED'
