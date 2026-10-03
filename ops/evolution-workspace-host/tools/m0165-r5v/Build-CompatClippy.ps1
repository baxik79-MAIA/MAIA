[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RustSourceRoot,
    [Parameter(Mandatory)][string]$OutputRoot,
    [string]$ToolchainRoot = 'C:\MAIA\restricted-verifier-depot\toolchains\stable-x86_64-pc-windows-msvc'
)
$ErrorActionPreference = 'Stop'
$expectedCommit = '48a229ceaefd4985c50990b14116b6d856af0985'
$sourceRoot = [IO.Path]::GetFullPath($RustSourceRoot)
$outputRoot = [IO.Path]::GetFullPath($OutputRoot)
$patch = Join-Path $PSScriptRoot 'compat-clippy.patch'
$lockSource = Join-Path $PSScriptRoot 'compat-clippy.Cargo.lock'
$git = (Get-Command git.exe -ErrorAction Stop).Source
$cargo = Join-Path $ToolchainRoot 'bin\cargo.exe'
$rustc = Join-Path $ToolchainRoot 'bin\rustc.exe'
$clippyPath = Join-Path $sourceRoot 'src\tools\clippy\clippy_config\src\conf.rs'
if (-not (Test-Path -LiteralPath $patch -PathType Leaf) -or -not (Test-Path -LiteralPath $lockSource -PathType Leaf)) { throw 'Committed Clippy patch or lockfile is missing.' }
if (-not (Test-Path -LiteralPath $cargo -PathType Leaf) -or -not (Test-Path -LiteralPath $rustc -PathType Leaf)) { throw 'The pinned 1.98.1 host toolchain is incomplete.' }
$actualCommit = (& $git -C $sourceRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $actualCommit -ne $expectedCommit) { throw "Expected rust-lang/rust commit $expectedCommit; found $actualCommit." }
$sourceStatus = @(& $git -C $sourceRoot status --porcelain --untracked-files=all)
if ($sourceStatus.Count -ne 0) { throw 'Use a clean Rust source checkout; refusing to overwrite pre-existing source changes.' }
& $git -C $sourceRoot apply --check $patch
if ($LASTEXITCODE -ne 0) { throw 'Compatibility patch does not apply cleanly.' }
if (Test-Path -LiteralPath $outputRoot) { throw 'OutputRoot already exists; choose a fresh build directory.' }
& $git -C $sourceRoot apply $patch
if ($LASTEXITCODE -ne 0) { throw 'Could not apply the reviewed compatibility patch.' }
$expectedConfHash = 'DB8FC462B4542BBBB3C0429052C0E7A16A0A329CC6E7CFB011A6C6A5B23A9EA4'
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $clippyPath).Hash -ne $expectedConfHash) { throw 'Patched Clippy config source hash differs from the reviewed source.' }
$patchedStatus = @(& $git -C $sourceRoot status --porcelain --untracked-files=all)
if ($patchedStatus.Count -ne 1 -or $patchedStatus[0] -notmatch 'src/tools/clippy/clippy_config/src/conf\.rs$') { throw 'Patch changed an unexpected Rust source path.' }
New-Item -ItemType Directory -Path $outputRoot | Out-Null
$standalone = Join-Path $outputRoot 'source'
New-Item -ItemType Directory -Path $standalone | Out-Null
$robocopy = Join-Path $env:WINDIR 'System32\robocopy.exe'
& $robocopy (Join-Path $sourceRoot 'src\tools\clippy') $standalone /E /COPY:DAT /R:1 /W:1 /XD target .git
if ($LASTEXITCODE -ge 8) { throw "Could not stage Clippy source; robocopy exit=$LASTEXITCODE." }
Copy-Item -LiteralPath $lockSource -Destination (Join-Path $standalone 'Cargo.lock')
$target = Join-Path $outputRoot 'target'
$cargoHome = Join-Path $outputRoot 'cargo-home'
New-Item -ItemType Directory -Force -Path $target,$cargoHome | Out-Null
$old = @{}
foreach ($name in @('PATH','CARGO_HOME','CARGO_TARGET_DIR','RUSTC','RUSTC_BOOTSTRAP','RUSTUP_HOME','RUSTUP_TOOLCHAIN','SYSROOT')) { $old[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
try {
    $env:PATH = (Join-Path $ToolchainRoot 'bin') + ';' + $old['PATH']
    $env:CARGO_HOME = $cargoHome
    $env:CARGO_TARGET_DIR = $target
    $env:RUSTC = $rustc
    $env:RUSTC_BOOTSTRAP = '1'
    $env:RUSTUP_HOME = Split-Path (Split-Path $ToolchainRoot -Parent) -Parent
    $env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-msvc'
    $env:SYSROOT = $ToolchainRoot
    & $cargo build --manifest-path (Join-Path $standalone 'Cargo.toml') --locked --release --bin cargo-clippy --bin clippy-driver
    if ($LASTEXITCODE -ne 0) { throw "Compatibility Clippy build failed; cargo exit=$LASTEXITCODE." }
} finally {
    foreach ($name in $old.Keys) { [Environment]::SetEnvironmentVariable($name,$old[$name],'Process') }
}
$bundle = Join-Path $outputRoot 'bundle'
New-Item -ItemType Directory -Path $bundle | Out-Null
foreach ($name in @('cargo-clippy.exe','clippy-driver.exe')) {
    $built = Join-Path $target "release\$name"
    if (-not (Test-Path -LiteralPath $built -PathType Leaf)) { throw "Build output missing: $built" }
    Copy-Item -LiteralPath $built -Destination $bundle
}
$dlls = @(Get-ChildItem -LiteralPath (Join-Path $ToolchainRoot 'bin') -File | Where-Object { $_.Name -like 'rustc_driver-*.dll' -or $_.Name -like 'std-*.dll' })
if (-not ($dlls.Name -like 'rustc_driver-*.dll') -or -not ($dlls.Name -like 'std-*.dll')) { throw 'Matching rustc_driver/std runtime DLL pair is missing from the pinned toolchain.' }
foreach ($dll in $dlls) { Copy-Item -LiteralPath $dll.FullName -Destination $bundle }
Copy-Item -LiteralPath $patch -Destination (Join-Path $bundle 'compat.patch')
$files = @(Get-ChildItem -LiteralPath $bundle -File | Sort-Object Name | ForEach-Object { [ordered]@{ name=$_.Name; bytes=$_.Length; sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash } })
$manifest = [ordered]@{
    purpose = 'M0.16.5 proof-only Clippy config-path compatibility; not stock Clippy'
    rust_repository = 'https://github.com/rust-lang/rust.git'
    rust_source_commit = $expectedCommit
    rust_tag = '1.98.1'
    rust_clippy_version = '0.1.98'
    cargo_lock_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $standalone 'Cargo.lock')).Hash
    config_source_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $standalone 'clippy_config\src\conf.rs')).Hash
    patch_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $patch).Hash
    files = $files
}
$manifestPath = Join-Path $bundle 'manifest.json'
[IO.File]::WriteAllText($manifestPath,($manifest | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
Write-Output "BUNDLE=$bundle"
Write-Output "MANIFEST=$manifestPath"