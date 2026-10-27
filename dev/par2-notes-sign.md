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

## Left, and why

- C-security-007: EC (ECDH) recipients are still refused (RSA key transport only); Acrobat accepts both.
- C-sign-013: smart cards and tokens need PKCS#11 (a native library loaded at run time) or the platform stores
  (macOS Keychain, Windows certificate store; PdfCraft has both readers, behind those OSes). Not wired here: needs a
  Mac and a Windows machine to build and test.
- C-sign-004 (camera capture), C-sign-011 (request signatures, Acrobat's cloud), C-sign-015 (trusted time stamp
  from a TSA), C-sign-016 (OCSP/CRL fetch), C-forms-016 (distribute and track): each needs a camera driver or the
  network; out of scope by Nassim's decision (no network features) or needing a non-pure dependency.
- C-forms-011: a barcode field needs a QR/PDF417 encoder; none is in the offline registry, and writing one was not
  possible in this round.

## For the orchestrator

- rustc on sheaf-app peaked at 8.0 GB RSS here (measured 27/10, generated Slint main.rs ~50 MB): `wbuild.sh`'s 6 GB
  cap OOM-killed it three times (also at 8 GB), so this branch's app, app tests and clippy were built under a 10 GB
  cap (`dev/.tmp/build-app.sh`, `dev/.tmp/cargo10.sh`, same flags otherwise). Other workers' rustc showed 7.3 GB.

## New dependencies

None.
