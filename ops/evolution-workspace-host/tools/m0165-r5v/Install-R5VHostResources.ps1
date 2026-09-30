[CmdletBinding()]
param([string]$DepotRoot = 'C:\MAIA\restricted-verifier-depot')
$ErrorActionPreference = 'Stop'
$depot = [IO.Path]::GetFullPath($DepotRoot)
if (-not $env:LOCALAPPDATA -or -not [IO.Path]::IsPathRooted($env:LOCALAPPDATA)) { throw 'LOCALAPPDATA is unavailable; refusing to register host resources.' }
$hostRoot = Join-Path $env:LOCALAPPDATA 'MAIA\RestrictedVerifierHost'
$config = Join-Path $hostRoot 'resources-root.txt'
$validator = Join-Path $PSScriptRoot 'Test-R5VHostResources.ps1'
& $validator -DepotRoot $depot
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$userSid = $identity.User
$systemSid = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$adminsSid = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
$allowed = @($systemSid.Value, $adminsSid.Value, $userSid.Value)
if (-not (Test-Path -LiteralPath $hostRoot -PathType Container)) {
    New-Item -ItemType Directory -Path $hostRoot | Out-Null
    $dirAcl = Get-Acl -LiteralPath $hostRoot
    $dirAcl.SetAccessRuleProtection($true, $false)
    $dirAcl.Access | ForEach-Object { [void]$dirAcl.RemoveAccessRuleSpecific($_) }
    foreach ($sid in @($systemSid, $adminsSid, $userSid)) {
        $rule = [Security.AccessControl.FileSystemAccessRule]::new($sid, [Security.AccessControl.FileSystemRights]::FullControl, [Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [Security.AccessControl.InheritanceFlags]::ObjectInherit, [Security.AccessControl.PropagationFlags]::None, [Security.AccessControl.AccessControlType]::Allow)
        [void]$dirAcl.AddAccessRule($rule)
    }
    Set-Acl -LiteralPath $hostRoot -AclObject $dirAcl
}
$dirAcl = Get-Acl -LiteralPath $hostRoot
if (-not $dirAcl.AreAccessRulesProtected -or $dirAcl.Owner -ne $identity.Name) { throw 'Host resource root ACL is not protected for the current host user.' }
foreach ($rule in $dirAcl.Access) {
    $sidText = $rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value
    if ($sidText -notin $allowed -or $rule.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow -or ($rule.FileSystemRights -band [Security.AccessControl.FileSystemRights]::FullControl) -ne [Security.AccessControl.FileSystemRights]::FullControl) { throw 'Unexpected host resource root ACL entry.' }
}
$sourceRustfmt = Join-Path $depot 'compat\rustfmt-r5\bundle-9f1ccda00d9cc632\rustfmt-r5-compat.exe'
$runtimeRoot = Join-Path $hostRoot 'resources\rustfmt-r5'
if (-not (Test-Path -LiteralPath $runtimeRoot -PathType Container)) { New-Item -ItemType Directory -Path $runtimeRoot | Out-Null }
$runtimeRustfmt = Join-Path $runtimeRoot 'rustfmt-r5-compat.exe'
$expectedRustfmt = '9F1CCDA00D9CC6321D529853ABC77733DF375C0087CE331AD27CD07E8BF69BFF'
if ((Test-Path -LiteralPath $runtimeRustfmt -PathType Leaf) -and (Get-FileHash -Algorithm SHA256 -LiteralPath $runtimeRustfmt).Hash -ne $expectedRustfmt) { throw 'Existing per-user Rustfmt executable differs from the pinned proof bundle; refusing replacement.' }
if (-not (Test-Path -LiteralPath $runtimeRustfmt -PathType Leaf)) { Copy-Item -LiteralPath $sourceRustfmt -Destination $runtimeRustfmt }
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $runtimeRustfmt).Hash -ne $expectedRustfmt) { throw 'Per-user Rustfmt copy failed pinned hash validation.' }
$sourceClippy = Join-Path $depot 'compat\clippy-r5v-rust-1.98.1-48a229cea'
$runtimeClippy = Join-Path $hostRoot 'resources\clippy-r5'
if (-not (Test-Path -LiteralPath $runtimeClippy -PathType Container)) { New-Item -ItemType Directory -Path $runtimeClippy | Out-Null }
$clippyPins = @{
    'cargo-clippy.exe' = '96A96C77099C22A1CF306C74626C7EC81EFD3A639B13AB485840063ECC22BCEB'
    'clippy-driver.exe' = 'A37DAF159E0CDC9B7A71875A4BD55904B6869E93D595EF155A2AB88734483CBC'
    'compat.patch' = 'E42793BCE501923C2FD1FB964AF7F661AA181C5A9AEBE6629DFA56B2253159B9'
    'rustc_driver-573e106f78c6e3e0.dll' = '50B9168C1D7BF98A7C4417EED0C8098DAAB7033B13EBB0BCA752945708EA9EC7'
    'std-44a584f44bc3dd65.dll' = '8DD8EA258A3561A499C438B322B218AF3CB2F4DB1029205ADC94D58FD5B0615C'
}
$clippyFiles = @('manifest.json') + @($clippyPins.Keys)
foreach ($name in $clippyFiles) {
    $source = Join-Path $sourceClippy $name
    $destination = Join-Path $runtimeClippy $name
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Pinned Clippy source file is missing: $source" }
    if ($clippyPins.ContainsKey($name)) {
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash -ne $clippyPins[$name]) { throw "Pinned Clippy source hash differs: $source" }
    }
    if (Test-Path -LiteralPath $destination -PathType Leaf) {
        if ($clippyPins.ContainsKey($name) -and (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash -ne $clippyPins[$name]) { throw "Existing per-user Clippy file differs from pin; refusing replacement: $destination" }
        if (-not $clippyPins.ContainsKey($name) -and (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash) { throw "Existing Clippy manifest differs from validated source; refusing replacement: $destination" }
    } else {
        Copy-Item -LiteralPath $source -Destination $destination
    }
}
foreach ($name in $clippyPins.Keys) {
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $runtimeClippy $name)).Hash -ne $clippyPins[$name]) { throw "Per-user Clippy staging failed hash validation: $name" }
}
$expected = $depot.TrimEnd('\')
if (Test-Path -LiteralPath $config -PathType Leaf) {
    $current = [IO.File]::ReadAllText($config).Trim()
    if (-not [string]::Equals($current, $expected, [StringComparison]::OrdinalIgnoreCase)) { throw "A different host resource root is already registered at $config; refusing to overwrite it." }
} else {
    $bytes = [Text.Encoding]::UTF8.GetBytes($expected + [string][char]10)
    $stream = [IO.File]::Open($config, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try { $stream.Write($bytes, 0, $bytes.Length); $stream.Flush($true) } finally { $stream.Dispose() }
}
$fileAcl = Get-Acl -LiteralPath $config
foreach ($rule in $fileAcl.Access) {
    $sidText = $rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value
    if ($sidText -notin $allowed -or $rule.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow) { throw 'Unexpected host config ACL entry.' }
}
$actual = [IO.File]::ReadAllText($config).Trim()
if (-not [string]::Equals($actual, $expected, [StringComparison]::OrdinalIgnoreCase)) { throw 'Host resource root registration did not round-trip.' }
Write-Output "HOST_RESOURCE_ROOT=$actual"
Write-Output "HOST_CONFIG=$config"
Write-Output "HOST_RUSTFMT_RUNTIME=$runtimeRustfmt; SHA256=$expectedRustfmt"
Write-Output "HOST_CLIPPY_RUNTIME=$runtimeClippy; files=$($clippyFiles.Count); hashes pinned"
Write-Output "HOST_CONFIG_OWNER=$($identity.Name); host-user scoped; administrator elevation not required"
