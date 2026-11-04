# SignPath Foundation application draft

**Status: prepared, not submitted.** The maintainer must submit the form and
accept the program's terms using their own account. Do not claim approval or
upload personal credentials to this repository.

Apply at https://signpath.org/apply. Suggested project details:

| Field | Value |
| --- | --- |
| Project | Navegante Desktop (repository: NaveganteWeb) |
| Maintainer | Tiago Dias, GitHub `tiagofdias` |
| Source | https://github.com/tiagofdias/NaveganteWeb |
| License | MIT — https://github.com/tiagofdias/NaveganteWeb/blob/main/LICENSE |
| Homepage | https://navegante-web-xi.vercel.app/ |
| Existing binary release | https://github.com/tiagofdias/NaveganteWeb/releases/tag/v2.0.0 |
| Build workflow | https://github.com/tiagofdias/NaveganteWeb/actions/workflows/native-windows.yml |
| Code signing policy | https://github.com/tiagofdias/NaveganteWeb/blob/main/docs/code-signing-policy.md |
| Privacy policy | https://github.com/tiagofdias/NaveganteWeb/blob/main/docs/privacy.md |
| Author/reviewer/approver | https://github.com/tiagofdias |
| Platform and artifact | Windows x64, native Rust executable `Navegante-Web.exe` |

## Suggested description

Navegante Desktop is a free, independent MIT-licensed Windows application that
lets a user read and load their physical Navegante public transport card through
a local USB PC/SC reader. It is written in Rust with an egui/eframe interface;
it does not embed a browser, Python runtime or local HTTP server.

The user explicitly starts reading their card, sees its titles and balance,
selects a product and reviews the price before requesting payment. Transport
data, prices, payment status and signed loading authorization come from the
official services. The application does not generate its own loading
authorization, bypass eligibility checks or accept arbitrary card commands.
Physical card writing requires both official authorization and user confirmation.

We request free signing for our own Windows executable so ordinary users can
identify its publisher and verify its origin. The project is newly released;
we do not claim an established adoption level or signing reputation. We
understand that acceptance is discretionary and are prepared to address the
foundation's review requirements.

Builds run on GitHub-hosted Windows runners with a pinned Rust compiler, locked
dependencies, pinned actions and financial/protocol tests. An unsigned build is
uploaded as a workflow artifact before any signing request. The maintainer will
manually approve production signing requests in SignPath. The application has no
tracking or proprietary backend/database operated by this project. It communicates
with transport/payment services only to perform actions requested by the user.

The application is not affiliated with TML or the Navegante brand owner. No
proprietary Android APK, extracted binary or third-party certificate is included
in the distributed executable.

## Maintainer checklist before submitting

- Confirm the CI run succeeds and that the owner controls the repository.
- Enable or confirm GitHub MFA; enable SignPath MFA when the account is created.
- Review dependency licenses and any notices required in the distributed binary
  package; MIT applies to original project code, not to third-party components.
- Review the foundation's current terms and privacy requirements.
- Supply the applicant/contact details directly in the provider's form.

## After acceptance

1. Configure the SignPath project and GitHub integration according to their
   onboarding instructions; grant access only to this repository.
2. Create the ZIP artifact configuration described in [signing.md](signing.md).
3. Configure the maintainer as author/reviewer and production signing approver.
4. Store the API token as a GitHub Actions secret and the nonsecret identifiers
   as the repository variables listed in `signing.md`.
5. Record the expected certificate thumbprint from SignPath and set
   `SIGNPATH_CERTIFICATE_THUMBPRINT`.
6. Update the public policy status and required provider attribution to reflect
   actual acceptance. Dispatch a signing build and approve it in SignPath.
7. Verify the returned signature, timestamp, native UI and checksum before
   publishing a new versioned direct download.
