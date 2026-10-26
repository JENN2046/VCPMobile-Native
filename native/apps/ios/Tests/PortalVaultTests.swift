import XCTest
import Security
@testable import VCPNative

final class PortalVaultTests: XCTestCase {
    private let a = "https://a.example.test"
    private let b = "https://b.example.test"
    private func issued(_ origin: String) throws -> SessionCredential {
        try SessionCredential.fromServerCookie(origin: origin, responseOrigin: origin,
            setCookie: "__Secure-vcp_mobile=\(String(repeating: "A", count: 43)); Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600")
    }
    private func service() -> String { "org.vcptoolbox.nativeclient.test.\(UUID().uuidString)" }
    private func removeService(_ service: String) {
        SecItemDelete([kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service] as CFDictionary)
    }
    func testRoundtripAcrossInstancesPreservesExpiry() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service: name)
        let original = try issued(a)
        try await vault.save(original)
        let restored = try await PortalSessionVault(service: name).load(a)
        XCTAssertNotNil(restored)
        XCTAssertTrue(try original.secretBytesForVault() == restored!.secretBytesForVault())
        XCTAssertEqual(original.expiresAtMs(), restored!.expiresAtMs())
    }
    func testCanonicalOriginAndPortIsolation() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service: name)
        try await vault.save(issued(a))
        let restored = try await vault.load("https://A.EXAMPLE.TEST:443")
        XCTAssertNotNil(restored)
        let wrongPort = try await vault.load("https://a.example.test:8443")
        let wrongOrigin = try await vault.load(b)
        XCTAssertNil(wrongPort)
        XCTAssertNil(wrongOrigin)
    }
    func testKeychainAttributesAreDeviceOnlyAndNotSynchronizable() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service: name)
        try await vault.save(issued(a))
        var item: CFTypeRef?
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: name, kSecAttrAccount as String: try credentialStorageKey(origin: a),
            kSecReturnAttributes as String: true, kSecMatchLimit as String: kSecMatchLimitOne]
        XCTAssertEqual(SecItemCopyMatching(query as CFDictionary, &item), errSecSuccess)
        let attrs = try XCTUnwrap(item as? [String: Any])
        XCTAssertEqual(attrs[kSecAttrAccessible as String] as? String,
                       kSecAttrAccessibleWhenUnlockedThisDeviceOnly as String)
        XCTAssertNotEqual(attrs[kSecAttrSynchronizable as String] as? Bool, true)
    }
    func testCorruptRecordIsRejectedAndRetained() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service: name)
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: name, kSecAttrAccount as String: try credentialStorageKey(origin: a),
            kSecValueData as String: Data("broken synthetic record".utf8),
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecAttrSynchronizable as String: false]
        XCTAssertEqual(SecItemAdd(query as CFDictionary, nil), errSecSuccess)
        var rejectedCorruption = false
        do { _ = try await vault.load(a) } catch { rejectedCorruption = true }
        XCTAssertTrue(rejectedCorruption, "Corrupt record was accepted")
        var read = query
        read.removeValue(forKey: kSecValueData as String)
        read[kSecReturnData as String] = true
        var item: CFTypeRef?
        XCTAssertEqual(SecItemCopyMatching(read as CFDictionary, &item), errSecSuccess)
        XCTAssertNotNil(item)
    }
    func testCredentialCannotMoveToAnotherOrigin() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service: name)
        let original = try issued(a)
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: name, kSecAttrAccount as String: try credentialStorageKey(origin: b),
            kSecValueData as String: Data(try original.secretBytesForVault()),
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecAttrSynchronizable as String: false]
        XCTAssertEqual(SecItemAdd(query as CFDictionary, nil), errSecSuccess)
        var rejectedWrongOrigin = false
        do { _ = try await vault.load(b) } catch { rejectedWrongOrigin = true }
        XCTAssertTrue(rejectedWrongOrigin, "Cross-origin record was accepted")
    }
    func testRemovalIsScopedAndUnpairedReadNeverRequestsNetwork() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service: name)
        try await vault.save(issued(a))
        try await vault.save(issued(b))
        try await vault.remove(a)
        let removed = try await vault.load(a)
        let remaining = try await vault.load(b)
        XCTAssertNil(removed)
        XCTAssertNotNil(remaining)
        do { _ = try await vault.fetchSession(a); XCTFail("Unpaired read was allowed") }
        catch PortalVaultError.notPaired {}
        catch { XCTFail("Unexpected error class") }
    }
    func testUnpairedHistoryDoesNotUseNetwork() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service: name)
        let expected = NativeSessionInfo(entryId: String(repeating: "a", count: 64),
            sessionMarker: String(repeating: "b", count: 64), topicKeys: ["topic_a"], selectedTopic: nil)
        do {
            _ = try await vault.fetchHistory(a, expected: expected, topicKey: "topic_a", after: 0)
            XCTFail("Unpaired history must not reach HTTPS")
        } catch PortalVaultError.notPaired {
        } catch {
            XCTFail("Unexpected history preflight error class")
        }
        let missing = try await vault.load(a)
        XCTAssertNil(missing)
    }

}
