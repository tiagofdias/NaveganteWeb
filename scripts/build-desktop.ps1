param(
    [switch]$Sign,
    [string]$CertificateThumbprint = $env:NAVEGANTE_SIGNING_THUMBPRINT
)
$ErrorActionPreference = 'Stop'
if ($Sign -and -not $CertificateThumbprint) { throw 'Supply -CertificateThumbprint or NAVEGANTE_SIGNING_THUMBPRINT for a signed release.' }
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $cargoBin = Join-Path $env:USERPROFILE '.cargo/bin'
        if (Test-Path "$cargoBin/cargo.exe") { $env:Path = "$cargoBin;$env:Path" }
    }
    # Optional portable compiler used by this workspace. Standard MSVC Rust builds
    # use the installed Visual Studio Build Tools without this fallback.
    $portableGcc = Join-Path $projectRoot 'analysis/tools/rust/w64devkit/bin'
    if ((rustc -vV) -match 'host: x86_64-pc-windows-gnu' -and (Test-Path "$portableGcc/gcc.exe")) {
        $env:Path = "$portableGcc;$env:Path"
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = 'gcc'
        $env:RUSTFLAGS = '-C link-self-contained=yes'
    }
    cargo build --locked --release --manifest-path native/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'Falhou a compilação Rust.' }
    New-Item -ItemType Directory -Force release | Out-Null
    Copy-Item -LiteralPath native/target/release/navegante-desktop.exe -Destination release/Navegante-Web.exe
    Get-FileHash -Algorithm SHA256 -LiteralPath release/Navegante-Web.exe
    if ($Sign) {
        & "$PSScriptRoot/sign-desktop.ps1" -CertificateThumbprint $CertificateThumbprint
    } else {
        Write-Output 'Unsigned development build. For public signing use -Sign -CertificateThumbprint <thumbprint>.'
    }
} finally {
    Pop-Location
}
