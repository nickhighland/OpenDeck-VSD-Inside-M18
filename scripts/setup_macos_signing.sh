#!/bin/bash
# One-time setup for signed and notarized macOS releases (docs/MACOS-SIGNING.md).
#
# Run it yourself in Terminal. It exports your Developer ID certificate (macOS
# asks you to allow that), asks for an app-specific password, checks it with
# Apple, and saves both as the repository's GitHub Actions secrets. Nothing is
# printed or left on disk.
set -euo pipefail

repo="nickhighland/OpenDeck-VSD-Inside-M18"
here="$(cd "$(dirname "$0")" && pwd)"

command -v gh > /dev/null || { echo "Install the GitHub CLI first: brew install gh"; exit 1; }
gh auth status > /dev/null 2>&1 || { echo "Sign in to the GitHub CLI first: gh auth login"; exit 1; }

identity="$(security find-identity -v -p codesigning | grep '"Developer ID Application:' | head -n 1 || true)"
if [ -z "$identity" ]; then
	echo "No Developer ID Application certificate is in your keychain."
	echo "Create one in Xcode: Settings > Accounts > Manage Certificates > + > Developer ID Application."
	exit 1
fi
fingerprint="$(awk '{ print $2 }' <<< "$identity")"
name="$(sed -E 's/^[^"]*"(.*)"$/\1/' <<< "$identity")"
team_id="$(sed -E 's/.*\(([A-Z0-9]{10})\)$/\1/' <<< "$name")"
echo "Certificate: $name"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo
echo "Exporting the certificate for GitHub. macOS asks for your login password to allow it."
swiftc -O -o "$work/export-signing-identity" "$here/export_signing_identity.swift"
export_password="$(uuidgen)-$(uuidgen)"
printf '%s\n' "$export_password" | "$work/export-signing-identity" "$fingerprint" "$work/certificate.p12"

echo
echo "Notarization uses an app-specific password: create one at https://account.apple.com"
echo "under Sign-In and Security > App-Specific Passwords."
read -r -p "Apple ID email: " apple_id
read -r -s -p "App-specific password (typing is hidden): " apple_password
echo
echo "Checking the credentials with Apple..."
if ! xcrun notarytool history --apple-id "$apple_id" --password "$apple_password" --team-id "$team_id" > /dev/null 2> "$work/notarytool.log"; then
	echo "Apple did not accept these credentials:"
	cat "$work/notarytool.log"
	exit 1
fi

echo "Saving the GitHub secrets..."
base64 -i "$work/certificate.p12" | gh secret set MACOS_CERTIFICATE --repo "$repo"
printf '%s' "$export_password" | gh secret set MACOS_CERTIFICATE_PASSWORD --repo "$repo"
printf '%s' "$apple_id" | gh secret set APPLE_ID --repo "$repo"
printf '%s' "$apple_password" | gh secret set APPLE_PASSWORD --repo "$repo"
printf '%s' "$team_id" | gh secret set APPLE_TEAM_ID --repo "$repo"

echo
echo "Done. Releases are now signed with \"$name\" and notarized."
