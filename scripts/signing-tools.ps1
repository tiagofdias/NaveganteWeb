function Find-SignTool([string]$ExplicitPath) {
    if ($ExplicitPath) { return (Resolve-Path -LiteralPath $ExplicitPath).Path }
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    $portable = Join-Path $PSScriptRoot '../analysis/tools/signing/signtool.exe'
    if (Test-Path -LiteralPath $portable -PathType Leaf) { return (Resolve-Path -LiteralPath $portable).Path }
    $sdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
    $candidates = @(Get-ChildItem -LiteralPath $sdk -Directory -ErrorAction SilentlyContinue |
        Where-Object Name -Match '^10\.\d+\.\d+\.\d+$' |
        Sort-Object { [version]$_.Name } -Descending |
        ForEach-Object { Join-Path $_.FullName 'x64/signtool.exe' } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    if ($candidates.Count) { return $candidates[0] }
    throw 'Install the Windows SDK Signing Tools, or supply -SignToolPath. No signing tool was found.'
}

function Test-CodeSigningPurpose([System.Security.Cryptography.X509Certificates.X509Certificate2]$Certificate) {
    foreach ($extension in $Certificate.Extensions) {
        if ($extension -is [System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]) {
            foreach ($purpose in $extension.EnhancedKeyUsages) {
                if ($purpose.Value -eq '1.3.6.1.5.5.7.3.3') { return $true }
            }
        }
    }
    return $false
}
