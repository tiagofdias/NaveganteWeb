[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Fa-f0-9]{40}$')][string]$CertificateThumbprint,
    [string]$SignToolPath
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/signing-tools.ps1"
$file = (Resolve-Path -LiteralPath $Executable).Path
$signature = Get-AuthenticodeSignature -LiteralPath $file
if ($signature.Status -ne 'Valid' -or -not $signature.SignerCertificate) {
    throw "Release refused: Windows does not trust this executable's signature ($($signature.Status))."
}
if ($signature.SignerCertificate.Thumbprint -ne $CertificateThumbprint) {
    throw 'Release refused: the signing identity differs from the expected certificate.'
}
if (-not $signature.TimeStamperCertificate) {
    throw 'Release refused: the executable has no trusted timestamp.'
}
$tool = Find-SignTool $SignToolPath
& $tool verify /pa /all /tw $file
if ($LASTEXITCODE -ne 0) { throw 'Release refused: SignTool verification failed.' }
Write-Output "Verified Authenticode signature and timestamp: $($signature.SignerCertificate.Subject)"
