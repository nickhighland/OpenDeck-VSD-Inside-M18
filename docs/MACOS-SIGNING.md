# Signing and notarizing macOS releases

macOS ties privacy permissions such as **Accessibility** (needed for hotkeys, typed text, and media keys) to the app's code signature. An ad-hoc signature changes with every build, so after each update macOS treats the app as a different program: it still looks switched on in System Settings, but keys that send keystrokes are refused until the app is removed from the Accessibility list and added again.

Signing every release with the same **Developer ID** certificate keeps the signature's identity stable, so permissions survive updates. **Notarization** also lets Gatekeeper open the app without the "unidentified developer" warning.

## One-time setup

This needs an Apple Developer Program membership.

1. **Create the certificate.** In Xcode, open **Settings › Accounts**, sign in with the account that has the membership, select the team, click **Manage Certificates…**, then **+ › Developer ID Application**. Only the account holder can create it.
2. **Create an app-specific password** for notarization at [account.apple.com](https://account.apple.com), under **Sign-In and Security › App-Specific Passwords**.
3. **Run the setup script** in Terminal, from the repository:

   ```sh
   scripts/setup_macos_signing.sh
   ```

   It exports the certificate (macOS asks for your login password to allow it), asks for your Apple ID and the app-specific password, checks them with Apple, and saves five GitHub Actions secrets: `MACOS_CERTIFICATE`, `MACOS_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`. Nothing is printed or left on disk.

## What the publish workflow does

On the macOS job, the workflow imports the certificate into a temporary keychain and signs with it. Before bundling, `scripts/sign_macos_plugins.ts` signs the built-in plugins' programs, which Tauri would otherwise copy unsigned; then Tauri signs, notarizes, and staples the app.

- Without `MACOS_CERTIFICATE`, the build is ad-hoc signed, and the run shows a warning.
- Without the three notarization secrets, the build is signed but not notarized, and the run shows a warning.

## After the first signed release

Install it, then remove **OpenDeck VSD M18** from **System Settings › Privacy & Security › Accessibility** and add it again one last time. Later releases keep the permission.

## Local builds

`APPLE_SIGNING_IDENTITY="Developer ID Application" deno task tauri build` signs a local build with the certificate (choose **Always Allow** if codesign asks to use the key). Local builds are notarized only when `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` are set in the environment.

## Renewing

Developer ID certificates last five years. Changing your Apple ID password revokes app-specific passwords. In either case, run the setup script again.

## Without a developer account

A self-signed certificate also keeps permissions across updates, but Gatekeeper still warns on first launch. Create one in Keychain Access (**Certificate Assistant › Create a Certificate…**, Identity Type **Self-Signed Root**, Certificate Type **Code Signing**, and a 3650-day validity under **Let me override defaults**), export it as a `.p12` file, and add only the certificate secrets:

```sh
base64 -i "Certificate.p12" | gh secret set MACOS_CERTIFICATE --repo nickhighland/OpenDeck-VSD-Inside-M18
gh secret set MACOS_CERTIFICATE_PASSWORD --repo nickhighland/OpenDeck-VSD-Inside-M18
```
