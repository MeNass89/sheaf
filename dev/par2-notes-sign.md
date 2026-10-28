# Round 30, batch "sign" (par2/sign)

## Done

- C-security-007: More ▸ More tools ▸ Encrypt with certificates… picks recipients from their certificates
  (.cer/.crt/.pem/.der) or the user's own digital ID (.p12, full rights), each with Print, Copy, Edit, Comment,
  Fill forms and Change security boxes, and what is encrypted (all, all but metadata, attachments only); one CMS
  envelope per permission set, as Acrobat writes them. A certificate-encrypted PDF opened in Sheaf asks for the
  digital ID it was sent to (the last one used, or Choose…) and its password, then opens in place: the tab shows
  the file itself, the recipient's permissions hold (edits, printing and copying refuse as for a restricted
  password; Properties ▸ Security says what is denied), and Save writes the original again, encrypted for every
  recipient with the same key. The decrypted working copy sits in an owner-only folder, leaves with its tab and is
  swept at the next start if a session crashed. Acrobat's Encrypt with Certificate and its in-place opening
  (`certificate_recipients_keep_their_own_permissions_and_saved_changes_reopen_for_all`,
  `a_certificate_recipients_permissions_refuse_edits_printing_and_copying_after_saves_too`,
  `a_certificate_file_opens_in_place_saves_encrypted_for_every_recipient_and_holds_permissions`,
  `recipient_checkboxes_map_to_their_envelope_permissions`,
  `stale_working_copies_of_dead_sessions_are_swept_and_live_ones_kept`; smoke 320).
- C-security-008: certificate security policies beside the password ones: the Encrypt with certificates sheet saves
  its recipients, their permissions and the scope as a named policy, lists the saved ones (Use, delete), exports and
  imports them (public certificates only; a damaged file is refused whole). Acrobat's Security Policies ▸ Use public
  key certificates (`a_certificate_policy_round_trips_and_applies_with_each_recipients_permissions`).
- C-sign-013: Preferences ▸ Signatures ▸ Create a digital ID…: name, unit, organization, email, country, password
  (twice, 6 or more), "use it for certificate encryption too"; a self-signed RSA 2048 identity valid five years,
  saved as a .p12 with its .cer beside it to share, and kept in the list of IDs. Acrobat's Add Digital ID ▸ A new
  digital ID ▸ New PKCS#12 file (`a_created_digital_id_reopens_signs_and_encrypts_only_when_asked`; smoke 321, which also checks the .p12 opens in
  OpenSSL with key encipherment).
- C-sign-007: deleting one copy of a signature placed on several pages takes every copy, also those pulled inside a
  shorter or narrower page by the placement (the match now follows the placement rule instead of wanting the same
  geometry everywhere). iLovePDF's repeated signatures (`every_page_copies_pulled_inside_smaller_pages_still_go_together`).
- C-forms-011: Prepare form ▸ Barcode places Acrobat's barcode field: a read-only text field with Acrobat's paper
  metadata (`/PMD`: QR Code, error correction, field names), its value the other fields' data tab-delimited under
  their names, drawn as a QR Code (vector modules, four-module quiet zone) at the size dragged. The engine draws it
  again after every change to the form (typing, check boxes, reset, fields added or removed); the options bar sets
  the error correction (Low, Medium, Quartile, High). Acrobat's barcode field
  (`a_barcode_field_draws_the_forms_data_as_a_qr_code_and_follows_every_change`, which reads the rendered page back
  with a QR reader; smoke 322).
- C-sign-016: a signature's own revocation evidence, Acrobat's `adbe-revocationInfoArchival` signed attribute (the
  CRLs and OCSP responses Acrobat embeds when it signs), is now checked offline next to the catalog's DSS (which
  PdfCraft already checked: CRLs and OCSPs, verified revocation wins): a revoking CRL makes the signature invalid
  ("revoked (CRL in the signature, date)"), a covering good answer is said in the details
  (`revocation_evidence_inside_a_signature_is_checked_offline`). Nothing is fetched.

## Left, and why

- C-security-007: EC (ECDH) recipients are still refused (RSA key transport only); Acrobat accepts both.
- C-sign-013: smart cards and tokens need PKCS#11 (a native library loaded at run time) or the platform stores
  (macOS Keychain, Windows certificate store; PdfCraft has both readers, behind those OSes). Not wired here: needs a
  Mac and a Windows machine to build and test.
- C-sign-004 (camera capture), C-sign-011 (request signatures, Acrobat's cloud), C-sign-015 (trusted time stamp
  from a TSA), C-forms-016 (distribute and track): each needs a camera driver or the network; out of scope by
  Nassim's decision (no network features) or needing a non-pure dependency.
- C-sign-016: OCSP/CRL fetching waits on Nassim's decision; only the signer's certificate is checked against the
  evidence (Acrobat also checks the intermediate CAs).
- C-forms-011: QR Code only; Acrobat also offers PDF417 and Data Matrix (the `pdf417` crate has no licence, `rxing`
  is large). Error correction is chosen for new barcode fields; changing it on an existing one goes through the engine
  edit (`SetBarcode`), not yet a properties card; XML encoding and a custom calculation script are not offered.

## For the orchestrator

- rustc on sheaf-app peaked at 8.0 GB RSS here (measured 27/10, generated Slint main.rs ~50 MB): `wbuild.sh`'s 6 GB
  cap OOM-killed it three times (also at 8 GB), so this branch's app, app tests and clippy were built under a 10 GB
  cap (`dev/.tmp/build-app.sh`, `dev/.tmp/cargo10.sh`, same flags otherwise). Other workers' rustc showed 7.3 GB.

## New dependencies

- `qrcode` 0.14 (MIT/Apache-2.0, pure Rust, default features off: no image) in `third_party/pdfcraft-engine`.
- `rqrr` 0.11 (MIT/Apache-2.0 and ISC, pure Rust, default features off) as a sheaf-core dev-dependency, to read the
  QR Code back in the test.

# Round 34, batch "sign" (par2/sign, on main 50a04a8)

## Done

- C-security-007: EC recipients. A P-256 or P-384 certificate (key usage keyAgreement) is a recipient like an RSA
  one: its envelope is a CMS KeyAgreeRecipientInfo (RFC 5753: ephemeral-static ECDH, the X9.63 KDF with SHA-256 or
  SHA-384, AES-256 key wrap), next to RSA key transport in the same EnvelopedData; an EC digital ID opens the copy.
  One check (`certificate_security::recipient_problem`) serves the core and the sheet. OpenSSL opens our EC
  envelopes and we open OpenSSL's (`ec_envelopes_interoperate_with_openssl_both_ways`,
  `ec_recipients_reopen_their_copy_through_key_agreement`; smoke 320 now encrypts for an ECC digital ID and an RSA
  certificate, and opens the copy in place with the ECC one). Pure Rust: `p256`/`p384` (ecdh), `aes-kw`.
- C-sign-013: smart cards and tokens. Preferences ▸ Signatures ▸ Add a smart card or token… loads the card's PKCS #11
  module (`cryptoki`), lists its tokens' IDs without the PIN and keeps each as an RFC 7512 `pkcs11:` reference in
  the list of digital IDs ("Holder (token)"). The Sign panel offers the kept IDs, a card's among them; with a card
  the password field asks for its PIN, and the signature is made on the token (CKM_RSA_PKCS over a DigestInfo, or
  CKM_ECDSA), the key never leaving it. A wrong PIN says "wrong PIN"; the CLI takes the same reference in --p12.
  Real token in the tests: SoftHSM, keys generated on it (`a_digital_id_on_a_pkcs11_token_signs_a_pdf_with_its_pin`;
  smoke 371–372; CI installs softhsm2, and opensc for the steps' pkcs11-tool).

## Gaps

- Opening a certificate-encrypted PDF with a smart card (decrypting on the token) is refused with a clear message;
  the card signs only.

## C-sign-004 Signature from the camera or an iPhone (28/10, Opus, on the MBP)

- Gap: no camera capture (Preview: Signatures ▸ Camera / iPhone). Image upload with background removal existed.
- Change: Sign panel, Image mode, Mac only: Camera and iPhone or iPad. Camera runs ImageKit's IKPictureTaker
  (camera, files; wide 3:1 crop, 1500×500 at most) in a helper (`sheaf-app --camera-to <png> <title>`,
  src/capture_mac.rs); iPhone runs the Continuity Camera helper in `photo` mode (Take Photo, Add Sketch; a scanned
  page comes as a picture). The picture then goes through the upload path: sign::aspect check, then
  sign::clear_background (sign_ui::capture_image, off the UI thread). Info.plist: NSCameraUsageDescription.
  Hint text on a Mac: "PNG or JPEG, or a photo of your signature on white paper". French in sheaf-ui.po.
- Mac evidence (MBP, debug build):
  - `SHEAF_TEST_CAMERA_SHOT=/tmp/sheafcap/camera-panel.png sheaf-app --camera-to …` → the panel shown, titled
    "Signature from the camera", its content an NSRemoteView (macOS 27's picture taker is drawn by another process,
    so the PNG holds only the frame; `panel visible: true`).
  - `SHEAF_TEST_CAMERA_IMAGE=signature-photo.jpg sheaf-app --camera-to c.png` → PNG 1600×1000 (the saving path).
  - iPhone helper in `photo` mode: JPEG → r-sign.png 1600×1000; menu filled as in C-files-018.
- Needs a phone / a person: the live camera picture (the remote panel is not drawable over ssh; camera permission
  is asked on first use in the bundle) and the phone's Take Photo. The Sign panel's two buttons were not clicked on
  the Mac (layout checked by Slint compile only; same code path as Upload image after the helper).
- Self-grade F 9 · A 8 · P 8 (A: IKPictureTaker is ImageKit's generic picture panel, not Preview's dedicated
  signature camera with its guide line).
# 2026-10-28, C-sign-015 "Time stamp a document" (wip/timestamp, on dev/phase-4-5 40a76a3)

## What Acrobat does

Tools ▸ Certificates ▸ Time Stamp adds an RFC 3161 document time stamp: an invisible signature field whose value
is a `/DocTimeStamp` dictionary (`/SubFilter /ETSI.RFC3161`), its `/Contents` the TSA's token over the ByteRange,
saved as an incremental update. Preferences ▸ Signatures ▸ Time Stamp Servers keeps the server list (add, remove,
set default). A signature made while a default server is set carries a signature time stamp (PAdES B-T). The
Signatures panel shows "Document is time stamped", the authority and the time.

## What Sheaf does

- Core `sheaf_core::timestamp`: `HttpAuthority` posts a TimeStampReq (SHA-256 imprint, a fresh 63-bit nonce,
  certReq true) as `application/timestamp-query` (ureq, 10 s connect, 30 s overall, reply capped at 4 MiB) and
  checks the reply before returning it: HTTP 200, PKIStatus granted (a rejection is reported with its status
  string and failInfo names), the token's CMS signature and imprint (`parse_response`), and the nonce.
  `timestamp_document(input, authority, out)` writes the update, reads the file back as the Signatures panel does,
  and refuses (deleting the file) if the new stamp does not validate. Messages name the server's host:
  "could not be found: is this computer offline?", "refused the connection.", "did not answer in time.",
  "answered HTTP 500 instead of a time stamp.", "refused the request (status 2, rejection: …)", "sent a reply
  that is not a valid time stamp (…)". Signature time stamps go through the existing
  `sign::sign_with_timestamp` / `certify_with_timestamp` with the same authority.
- Engine patch `third_party/pdfcraft-engine/src/doc_timestamp.rs`: upstream's `timestamp_document` wrote the
  `/DocTimeStamp` dictionary with no field, so poppler (and Acrobat, which list from the form's fields) did not
  see it. It is now the value of an invisible field `TimestampN` (widget on page 1, Rect 0 0 0 0, Print+Locked,
  AcroForm `/Fields` and `/SigFlags 3`), as Acrobat writes it. PdfCraft's validator already reads field-held
  document time stamps.
- App: the Sign tool's options gain "Add a trusted time stamp" with the server list to pick from. Nothing placed
  and no certification: the job is Time Stamp Document. With a digital ID: the signature's own time stamp
  (and an invisible certification likewise). A signature without a certificate: signed, then a document time
  stamp over it. Preferences ▸ Signatures ▸ Time stamp servers: the list (default first, "Make default",
  "Remove"), an address field and "Add server" (http/https checked), the presets FreeTSA and DigiCert listed
  until the user changes the list. No server is contacted except when Sign runs with the box ticked.
  Document properties: a document time stamp reads "Signature N · document time stamp: Time stamp is valid /
  validity is unknown / invalid. Time … UTC, from the time stamp authority <CN>. <details>"; a signature with
  its own token adds "Time stamped … UTC". French for every new text.

## Tests

`crates/sheaf-core/tests/timestamp.rs`, no internet: a test CA and a TSA certificate it issued (EKU
timeStamping critical) made with openssl in the test, `openssl ts -reply` behind a tiny HTTP thread.
- `a_document_time_stamp_from_a_server_validates_everywhere`: the request carries sha256, a nonce, certReq
  (`openssl ts -query -text`); the original bytes are kept in front; `qpdf --check` clean; PdfCraft lists one
  document time stamp, authority "Sheaf Test TSA", its time, "not been modified", Unknown (test CA not trusted);
  `openssl ts -verify -data <ByteRange bytes> -in <Contents> -token_in -CAfile ca.pem` says "Verification: OK";
  `pdfsig` lists field Timestamp1 "Total document signed" (poppler 24 cannot validate ETSI.RFC3161:
  "Unable to validate this type of signature").
- `a_signed_document_stays_valid_once_time_stamped_and_a_signature_gets_its_own_stamp`: a signature with its
  token from the same server, then a document time stamp over it: pdfsig still says "Signature is Valid" for
  the first, the stamp covers the whole file, qpdf clean.
- `a_refused_or_bad_reply_writes_nothing_and_says_why`: rejection, garbage, HTTP 500, nothing listening: the
  message says which, no file written.
- Units: address checks and host names, rejection reasons (`timestamp::tests`); `sign_ui::tests::
  a_time_stamp_needs_a_server_address`; `run::tests::sign_job_…` builds Job::Timestamp with nothing placed.

## Not done

- No GUI smoke step and the new options and Preferences rows were not rendered and looked at.
- TSA authentication (user name and password, client certificate) and per-server policy OIDs: not offered.
- LTV for the time stamp (the TSA's revocation data in the DSS) is not added with it.
- Acrobat's own Signatures panel (a side pane with a tree) is Sheaf's Document properties list.
- Trusting the TSA's root is done from Preferences ▸ Signatures ▸ Trust a certificate…, as for signers; until
  then the stamp reads "validity is unknown" (Acrobat does the same with an untrusted TSA).

## Self-grade

F 8 (document time stamp, signature time stamp, server list, errors all work and are proved by outside
verifiers; TSA authentication and LTV missing). A 7 (follows the Sign options and Preferences rows' existing
look, but not rendered and checked by eye). P 9 (one request, bounded timeouts and reply size; the document is
written once, read back once).
