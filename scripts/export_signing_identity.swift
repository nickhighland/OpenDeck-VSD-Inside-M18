// Exports one code-signing identity from the keychain as a password-protected
// PKCS #12 file for scripts/setup_macos_signing.sh. `security export` can only
// export every identity in a keychain at once, which would upload unrelated
// private keys along with the Developer ID certificate.
//
// Usage: export-signing-identity <certificate SHA-1> <output.p12>
// The export password is read from standard input.

import CryptoKit
import Foundation
import Security

func fail(_ message: String) -> Never {
	FileHandle.standardError.write(Data((message + "\n").utf8))
	exit(1)
}

let arguments = CommandLine.arguments
guard arguments.count == 3 else { fail("usage: export-signing-identity <certificate SHA-1> <output.p12>") }
let wanted = arguments[1].uppercased()
let output = URL(fileURLWithPath: arguments[2])
guard let password = readLine(strippingNewline: true), !password.isEmpty else { fail("no export password on standard input") }

let query: [String: Any] = [
	kSecClass as String: kSecClassIdentity,
	kSecMatchLimit as String: kSecMatchLimitAll,
	kSecReturnRef as String: true,
]
var found: CFTypeRef?
guard SecItemCopyMatching(query as CFDictionary, &found) == errSecSuccess, let identities = found as? [SecIdentity] else {
	fail("the keychain has no signing identities")
}

func fingerprint(_ identity: SecIdentity) -> String? {
	var certificate: SecCertificate?
	guard SecIdentityCopyCertificate(identity, &certificate) == errSecSuccess, let certificate else { return nil }
	return Insecure.SHA1.hash(data: SecCertificateCopyData(certificate) as Data).map { String(format: "%02X", $0) }.joined()
}

guard let identity = identities.first(where: { fingerprint($0) == wanted }) else { fail("no identity has certificate \(wanted)") }

let passphrase = password as CFString
var parameters = SecItemImportExportKeyParameters()
parameters.version = UInt32(SEC_KEY_IMPORT_EXPORT_PARAMS_VERSION)
parameters.passphrase = Unmanaged.passUnretained(passphrase as AnyObject)
var exported: CFData?
let status = withExtendedLifetime(passphrase) { SecItemExport(identity, .formatPKCS12, [], &parameters, &exported) }
guard status == errSecSuccess, let exported else {
	fail("macOS did not export the certificate: \(SecCopyErrorMessageString(status, nil) as String? ?? "error \(status)")")
}
do {
	try (exported as Data).write(to: output, options: .withoutOverwriting)
} catch {
	fail("could not write \(output.path): \(error.localizedDescription)")
}
