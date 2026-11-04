param([string]$SignToolPath)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
. "$root/scripts/signing-tools.ps1"
$tool = Find-SignTool $SignToolPath
$certificate = (Get-AuthenticodeSignature -LiteralPath $tool).SignerCertificate
if (-not $certificate) { throw 'Use an authentic, signed Microsoft SDK SignTool for this test.' }
if (-not (Test-CodeSigningPurpose $certificate)) { throw 'The SDK signing certificate must be recognized as code signing.' }
$emptyCertificate = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2
if (Test-CodeSigningPurpose $emptyCertificate) { throw 'A certificate with no code-signing purpose must be rejected.' }
function Assert-Rejected([scriptblock]$Action, [string]$Expected) {
    try { & $Action; throw 'TEST: expected rejection did not occur.' }
    catch { if ($_.Exception.Message -notmatch $Expected) { throw } }
}
# Verify the success path with Microsoft's real signed, timestamped SDK binary.
# This does not create or install a certificate, or sign our application.
& "$root/scripts/verify-desktop-signature.ps1" -Executable $tool -CertificateThumbprint $certificate.Thumbprint -SignToolPath $tool
$unsigned = Join-Path $root 'native/target/release/navegante-desktop.exe'
if (-not (Test-Path -LiteralPath $unsigned)) { throw 'Build the Rust executable before this test.' }
$before = (Get-FileHash -LiteralPath $unsigned -Algorithm SHA256).Hash
Assert-Rejected { & "$root/scripts/verify-desktop-signature.ps1" -Executable $unsigned -CertificateThumbprint $certificate.Thumbprint -SignToolPath $tool } 'Windows does not trust'
Assert-Rejected { & "$root/scripts/verify-desktop-signature.ps1" -Executable $tool -CertificateThumbprint ('0' * 40) -SignToolPath $tool } 'identity differs'
Assert-Rejected { & "$root/scripts/sign-desktop.ps1" -Executable $unsigned -CertificateThumbprint ('0' * 40) -SignToolPath $tool } 'does not exist|Cannot find'
Assert-Rejected { & "$root/scripts/build-desktop.ps1" -Sign -CertificateThumbprint '' } 'Supply -CertificateThumbprint'
if ((Get-FileHash -LiteralPath $unsigned -Algorithm SHA256).Hash -ne $before) { throw 'Signing checks modified the unsigned application.' }
Write-Output 'PASS: real trusted signature/timestamp verification; unsigned, wrong-identity and missing-certificate releases rejected; executable unchanged.'
