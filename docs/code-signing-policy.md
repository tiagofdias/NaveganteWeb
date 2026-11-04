# Code signing policy

## Current status

The project is preparing an application to SignPath Foundation's free open-source
signing program. Acceptance and signing credentials are **pending**. The public
2.0.0 download is unsigned. An MIT license and a passing build do not mean that
SignPath has accepted or signed the project.

If approved, the attribution for that service will be:
“Free code signing provided by [SignPath.io](https://about.signpath.io),
certificate by [SignPath Foundation](https://signpath.org).”

## Team and approvals

- Author, committer and reviewer: [Tiago Dias / tiagofdias](https://github.com/tiagofdias).
- Release-signing approver: [Tiago Dias / tiagofdias](https://github.com/tiagofdias).
- Outside contributions require maintainer review before merging.
- Maintainers must use MFA on GitHub and SignPath before signing is enabled.
- Every production signing request requires explicit approval in SignPath.

Only artifacts built from this repository on GitHub-hosted Windows runners may
be submitted through the SignPath integration. The workflow uses a fixed Rust
toolchain, locked Cargo dependencies and actions pinned to commit identifiers.
Signing is disabled for ordinary pushes, pull requests and forks; the maintainer
must explicitly dispatch a signing build from `main` after approval and setup.
No private certificate key is placed in this repository.

Signing is restricted to the project's Windows executable. The SignPath artifact
configuration must restrict the ZIP to `Navegante-Web.exe`, the accompanying
MIT `LICENSE` and `THIRD-PARTY-NOTICES.txt`, sign only the executable, enforce `ProductName = Navegante Desktop`
and enforce matching file/product versions for the release. Configure this in
SignPath before enabling the workflow's signing option.

The downloaded result must pass Windows Authenticode verification, a trusted
timestamp check and an exact check against the approved certificate thumbprint.
The SHA-256 checksum is calculated after signing. Only those verified bytes may
be published under a new versioned release. Signing is not a guarantee that
SmartScreen will immediately trust a new file.

## Privacy

The application sends information to external services only as part of actions
requested by the user, such as reading the card, checking products, making a
purchase or confirming a load. See the [privacy policy](privacy.md) for details
and the services involved.

## Licensing and scope

Original project code is available under the [MIT license](../LICENSE).
Third-party dependencies retain their own licenses. The MIT license does not
grant rights to third-party trademarks or proprietary services. This is an
independent project and is not affiliated with TML or the Navegante brand owner.

The local scripts for an independently purchased Authenticode certificate remain
available as an optional alternative; they are not needed for the SignPath
Foundation application.
