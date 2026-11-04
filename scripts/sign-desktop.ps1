[CmdletBinding()]
param(
    [string]$Executable = "$PSScriptRoot/../release/Navegante-Web.exe",
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Fa-f0-9]{40}$')][string]$CertificateThumbprint,
    [uri]$TimestampUrl = 'http://time.certum.pl',
    [string]$SignToolPath
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/signing-tools.ps1"
if ($TimestampUrl.Scheme -notin @('http','https') -or -not $TimestampUrl.Host -or $TimestampUrl.UserInfo) {
    throw 'Supply an HTTP(S) RFC 3161 timestamp server URL without embedded credentials.'
}
$source = (Resolve-Path -LiteralPath $Executable).Path
if ([IO.Path]::GetExtension($source) -ne '.exe') { throw 'Select the Windows executable.' }
$certificate = Get-Item -LiteralPath "Cert:/CurrentUser/My/$CertificateThumbprint" -ErrorAction Stop
if (-not $certificate.HasPrivateKey) { throw 'The certificate has no accessible private key. Sign in to your signing provider first.' }
if ($certificate.NotAfter -le (Get-Date) -or $certificate.NotBefore -gt (Get-Date)) { throw 'The signing certificate is not currently valid.' }
if (-not (Test-CodeSigningPurpose $certificate)) { throw 'Select a certificate issued for code signing.' }
if ($certificate.Subject -eq $certificate.Issuer) { throw 'A self-signed certificate does not establish public publisher trust.' }
$tool = Find-SignTool $SignToolPath
$directory = Join-Path (Split-Path -Parent $source) 'signed'
New-Item -ItemType Directory -Force -Path $directory | Out-Null
$candidate = Join-Path $directory (([guid]::NewGuid().ToString('N')) + '.exe')
$destination = Join-Path $directory 'Navegante-Web.exe'
try {
    Copy-Item -LiteralPath $source -Destination $candidate
    # A cloud/HSM-backed certificate can be exposed through CurrentUser/My by
    # the provider's client. The key never enters the repository or command line.
    & $tool sign /s My /sha1 $CertificateThumbprint /fd SHA256 /tr $TimestampUrl.AbsoluteUri /td SHA256 /d 'Navegante Desktop' /du 'https://navegante-web-xi.vercel.app/' $candidate
    if ($LASTEXITCODE -ne 0) { throw 'Signing failed. No signed release was produced.' }
    & "$PSScriptRoot/verify-desktop-signature.ps1" -Executable $candidate -CertificateThumbprint $CertificateThumbprint -SignToolPath $tool
    Move-Item -LiteralPath $candidate -Destination $destination -Force
    $digest = (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $directory 'SHA256SUMS.txt'), "$digest  Navegante-Web.exe`n", [Text.Encoding]::ASCII)
    Write-Output "Signed release: $destination"
    Write-Output "SHA-256: $digest"
} finally {
    if (Test-Path -LiteralPath $candidate) { Remove-Item -LiteralPath $candidate }
}
