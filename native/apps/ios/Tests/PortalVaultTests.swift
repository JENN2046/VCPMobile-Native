import XCTest
import Security
import CryptoKit
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
        // This helper only receives UUID-scoped synthetic test services.
        if service.hasPrefix("org.vcptoolbox.nativeclient.test."),
           let support = try? FileManager.default.url(for:.applicationSupportDirectory,in:.userDomainMask,appropriateFor:nil,create:false) {
            let namespace = SHA256.hash(data:Data(service.utf8)).map { String(format:"%02x",$0) }.joined()
            try? FileManager.default.removeItem(at:support.appendingPathComponent("portal-pending-v1").appendingPathComponent(namespace))
        }
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

    func testUnpairedRecoveryDoesNotUseNetwork() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service: name)
        let expected = NativeSessionInfo(entryId: String(repeating: "a",count:64),
            sessionMarker:String(repeating:"b",count:64),topicKeys:["topic_a"],selectedTopic:nil)
        let fence = try NativeRecoveryFence(expected:expected,topicKey:"topic_a",viewGeneration:1)
        do {
            _ = try await vault.recoverPending(a,expected:expected,topic:"topic_a",generation:1,fence:fence)
            XCTFail("Unpaired recovery must not reach HTTPS")
        } catch PortalVaultError.notPaired {} catch { XCTFail("Unexpected recovery preflight error") }
    }
    func testPendingCheckpointSurvivesReopenCancellationAndCredentialRemoval() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service:name)
        try await vault.save(issued(a))
        let expected = NativeSessionInfo(entryId:String(repeating:"a",count:64),
            sessionMarker:String(repeating:"b",count:64),topicKeys:["topic_a"],selectedTopic:nil)
        let pending = try await vault.checkpointOriginal(a,expected:expected,topic:"topic_a",kind:.message,
            originalJson:"{ \"requestId\":\"native_pending_123\", \"text\":\"synthetic pending\" }")
        let reopened = PortalSessionVault(service:name)
        let saved = try await reopened.pendingOriginals(a,topic:"topic_a")
        XCTAssertEqual(saved,[pending])
        let namespace = SHA256.hash(data:Data(name.utf8)).map { String(format:"%02x",$0) }.joined()
        let support = try FileManager.default.url(for:.applicationSupportDirectory,in:.userDomainMask,appropriateFor:nil,create:false)
        let directory = support.appendingPathComponent("portal-pending-v1").appendingPathComponent(namespace)
            .appendingPathComponent(try credentialStorageKey(origin:a))
        XCTAssertEqual(try directory.resourceValues(forKeys:[.isExcludedFromBackupKey]).isExcludedFromBackup,true)
        let attrs = try FileManager.default.attributesOfItem(atPath:directory.appendingPathComponent("pending.sqlite").path)
        XCTAssertEqual(attrs[.protectionKey] as? FileProtectionType,.complete)
        let fence = try NativeRecoveryFence(expected:expected,topicKey:"topic_a",viewGeneration:2)
        fence.cancel()
        do {
            _ = try await reopened.recoverPending(a,expected:expected,topic:"topic_a",generation:2,fence:fence)
            XCTFail("Cancelled recovery must not reach HTTPS")
        } catch NativeRecoveryError.Cancelled {} catch { XCTFail("Unexpected cancellation error") }
        let retained = try await reopened.pendingOriginals(a,topic:"topic_a")
        XCTAssertEqual(retained,[pending])
        try await vault.remove(a)
        let afterRemoval = try await reopened.pendingOriginals(a,topic:"topic_a")
        XCTAssertEqual(afterRemoval,[pending])
        try FileManager.default.removeItem(at:directory.deletingLastPathComponent())
    }
    func testExpiredRecoveryRetainsCredentialAndPending() async throws {
        let name = service(); defer { removeService(name) }
        let vault = PortalSessionVault(service:name)
        let credential = try SessionCredential.fromServerCookie(origin:a,responseOrigin:a,
            setCookie:"__Secure-vcp_mobile=\(String(repeating:"B",count:43)); Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=5")
        try await vault.save(credential)
        let expected = NativeSessionInfo(entryId:String(repeating:"a",count:64),
            sessionMarker:String(repeating:"b",count:64),topicKeys:["topic_a"],selectedTopic:nil)
        let pending = try await vault.checkpointOriginal(a,expected:expected,topic:"topic_a",kind:.message,
            originalJson:"{\"requestId\":\"expired_pending_123\",\"text\":\"synthetic\"}")
        try await Task.sleep(nanoseconds:5_100_000_000)
        let fence = try NativeRecoveryFence(expected:expected,topicKey:"topic_a",viewGeneration:3)
        do {
            _ = try await vault.recoverPending(a,expected:expected,topic:"topic_a",generation:3,fence:fence)
            XCTFail("Expired recovery must not reach HTTPS")
        } catch SessionCredentialError.Expired {} catch { XCTFail("Unexpected expiry rejection") }
        let retained = try await vault.pendingOriginals(a,topic:"topic_a")
        XCTAssertEqual(retained,[pending])
        let query: [String:Any] = [kSecClass as String:kSecClassGenericPassword,kSecAttrService as String:name,
            kSecAttrAccount as String:try credentialStorageKey(origin:a),kSecReturnAttributes as String:true]
        var attributes:CFTypeRef?
        XCTAssertEqual(SecItemCopyMatching(query as CFDictionary,&attributes),errSecSuccess)
    }

}
