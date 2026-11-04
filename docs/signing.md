# Signing direct Windows downloads

The public 2.0 executable is currently unsigned. The scripts below prepare the
Authenticode release process; they do not issue a certificate or establish
SmartScreen reputation by themselves.

## Obtain the publisher identity

For an individual based in Portugal, Microsoft Azure Artifact Signing Public
Trust is not currently available: individual developers must be in the USA or
Canada. A certificate authority supporting individuals is required.

Certum offers cloud-backed certificates that work with Windows SignTool:

- [Standard Code Signing in the Cloud](https://shop.certum.eu/standard-code-signing-in-the-cloud.html): listed from EUR 209 on 4 November 2026. Confirm duration, taxes, availability and final price at checkout.
- [Open Source Code Signing in the Cloud](https://shop.certum.eu/open-source-code-signing-on-simplysign.html): listed from EUR 49, but the page currently reports out of stock. Eligibility must be confirmed with Certum. A public repository alone does not establish open-source licensing; this repository currently has no project license. Do not select a license without the owner's decision.

The certificate is issued in the developer's verified legal identity, rather than
an invented Navegante/TML publisher identity. Complete identity verification
directly with the provider. Keep ID documents, payment details, recovery codes
and private keys out of the repository and chat.

Install the provider's SimplySign mobile/desktop clients, activate the issued
certificate and connect it to the Windows current-user certificate store. Only
the publisher needs this setup. Users downloading the application do not.

Provider documentation:
[activation](https://support.certum.eu/en/how-to-activate-code-signing-simplysign/),
[SignTool integration](https://files.certum.eu/documents/manual_en/CS-Code_Signing_in_the_Cloud_Signtool_jarsigner_signing.pdf).

## Build, sign and verify

Install the Windows SDK Signing Tools. SignTool can also be supplied explicitly
with `-SignToolPath`. This workspace additionally detects its ignored portable
copy under `analysis/tools/signing/`, extracted from Microsoft's
`Microsoft.Windows.SDK.BuildTools` NuGet package.

List the installed public certificate identifiers:

```powershell
Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Select-Object Subject, Thumbprint, NotAfter
```

Sign a fresh build using the certificate's thumbprint (a public identifier):

```powershell
./scripts/build-desktop.ps1 -Sign -CertificateThumbprint YOUR_40_CHARACTER_THUMBPRINT
```

Or sign the already built executable:

```powershell
./scripts/sign-desktop.ps1 -CertificateThumbprint YOUR_40_CHARACTER_THUMBPRINT
```

The key stays with the cloud/HSM provider. The script selects the exact certificate
from `CurrentUser/My`, uses SHA-256 and an RFC 3161 timestamp, signs a temporary
copy and verifies both Windows trust and the expected certificate before
producing `release/signed/Navegante-Web.exe`. It then generates
`release/signed/SHA256SUMS.txt` from the signed bytes. It leaves the unsigned
development executable intact. It stops on missing/expired certificates,
self-signed certificates, signing failures and verification failures.

Before publishing, run:

```powershell
./scripts/verify-desktop-signature.ps1 -Executable release/signed/Navegante-Web.exe -CertificateThumbprint YOUR_40_CHARACTER_THUMBPRINT
powershell -NoProfile -File tests/native-ui.ps1 -Executable release/signed/Navegante-Web.exe
```

Publish the signed executable and its matching checksum together in a **new
versioned GitHub release**. Update the website only after checking the public
download's signature and hash. Do not overwrite the previous unsigned release
under the same version, publish the unsigned build as signed, or alter the
executable after signing. Keep using the same verified publisher identity for
subsequent versions.

## Verification of the signing tools

```powershell
powershell -NoProfile -File tests/signing.ps1
```

This uses Microsoft's genuinely signed SDK tool to test successful verification,
and rejects the unsigned Rust binary, wrong signing identity and missing signing
certificate. It neither creates a test publisher identity nor installs a root
certificate. Actual signing of our application still requires the issued key.

Signing displays the verified publisher and supports reputation across releases;
it does **not** guarantee the immediate removal of SmartScreen prompts.
[Microsoft's SmartScreen guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
and [eligibility](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart).
