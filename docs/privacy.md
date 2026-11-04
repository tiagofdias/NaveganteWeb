# Privacy policy

Updated 4 November 2026. Maintainer: [Tiago Dias](https://github.com/tiagofdias).
This policy describes the independent Navegante Desktop project, not the
official transport or payment providers.

## Desktop application

The application reads a selected PC/SC reader and card after the user chooses
to start. It does not upload card data merely because the application is open.
The official read protocol exchanges card responses with
`https://lisbon.tiptop.spiws.net/tiptop/` to obtain the card's details, titles and
balance. Selecting a product requests its available options and price there.

If the user confirms a purchase, the application sends the transaction session,
receipt e-mail, MB WAY phone number and optional tax number to
`https://pcgi.tmlmobilidade.pt/app/edge/`. It checks payment and loading status
with that service. Writing a title requires an authorization from the official
service and the user's explicit confirmation. Card data and purchase details
are not sent to SignPath.

The application does not operate its own database, analytics, advertising or
telemetry service. Card readings, contact details and transaction state stay
in the application's process memory and are discarded on exit. The user can
explicitly copy a transaction reference to the Windows clipboard. Provider-side
purchase records, receipts and payment processing are separate from this local
memory and can persist under the providers' own policies.

No card PIN is requested from the user. The read protocol can exchange an
encrypted verification value supplied by the official service; the application
does not ask the user to supply or save a PIN.

## Public website and demo

The website is hosted by Vercel and the executable is distributed through
GitHub Releases. Those hosts receive normal request information such as IP
addresses and user agents. The project adds no tracking cookies or analytics.
The web demo uses fictitious data in browser memory, does not request a reader,
does not call transport/payment APIs and makes no real transactions.

## External providers

- TML/Navegante and the payment provider process the transport and payment
  operations the user requests. Consult the official provider's terms and
  privacy information before purchasing.
- [Vercel privacy notice](https://vercel.com/legal/privacy-policy).
- [GitHub privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).
- [SignPath privacy policy](https://signpath.io/privacy-policy) applies to the
  maintainer's signing service account, if the project is accepted. Ordinary
  users do not need a signing-service account or client.

The desktop application is portable: closing it and deleting the executable
removes the application. Reader drivers are managed separately by Windows or
the hardware vendor.
