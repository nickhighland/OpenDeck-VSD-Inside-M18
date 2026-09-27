# Signing macOS releases with a stable certificate

macOS ties privacy permissions such as **Accessibility** (needed for hotkeys, typed text, and media keys) to the app's code signature. Release builds are ad-hoc signed unless a certificate is configured, and an ad-hoc signature changes with every build. After each update, macOS then treats the app as a different program: it still looks switched on in System Settings, but keys that send keystrokes are refused until the app is removed from the Accessibility list and added again.

Signing every release with the same certificate keeps the signature's identity stable, so permissions survive updates. A free self-signed certificate is enough for this. It does not remove the Gatekeeper warning on first launch; that needs an Apple Developer ID and notarization.

## One-time setup

1. **Create the certificate.** In Keychain Access, choose **Keychain Access › Certificate Assistant › Create a Certificate…**:
   - Name: `OpenDeck VSD M18 Signing`
   - Identity Type: **Self-Signed Root**
   - Certificate Type: **Code Signing**
   - Select **Let me override defaults**, continue, and set **Validity Period** to `3650` days so it does not expire after a year. Keep the other defaults and save it in the **login** keychain.
2. **Export it.** In Keychain Access, open **login › My Certificates**, right-click **OpenDeck VSD M18 Signing**, choose **Export…**, save it as a `.p12` file, and set a strong password. Keep the file and password somewhere safe, such as a password manager: a new certificate means granting permissions once more.
3. **Add it to GitHub.** From the folder with the exported file:

   ```sh
   base64 -i "OpenDeck VSD M18 Signing.p12" | gh secret set MACOS_CERTIFICATE --repo nickhighland/OpenDeck-VSD-Inside-M18
   gh secret set MACOS_CERTIFICATE_PASSWORD --repo nickhighland/OpenDeck-VSD-Inside-M18
   ```

   The second command asks for the export password.

The publish workflow then signs the macOS build with this certificate. Without the secrets it falls back to ad-hoc signing and says so in the run's warnings.

## After the first signed release

Install it, then remove **OpenDeck VSD M18** from **System Settings › Privacy & Security › Accessibility** and add it again one last time. Later releases signed with the same certificate keep the permission.

To sign local builds with the certificate too, run `APPLE_SIGNING_IDENTITY="OpenDeck VSD M18 Signing" deno task tauri build`.
