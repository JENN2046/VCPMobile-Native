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
    // Diagnostic-only candidate. Keep the existing Complete assertion below.
    // Never log paths, journal bytes, credentials, or arbitrary error strings.
    private func protectionProbe(_ url: URL, label: String) -> [FileAttributeKey: Any]? {
        let known = ["NSFileProtectionNone", "NSFileProtectionComplete",
            "NSFileProtectionCompleteUnlessOpen", "NSFileProtectionCompleteUntilFirstUserAuthentication",
            "NSURLFileProtectionNone", "NSURLFileProtectionComplete",
            "NSURLFileProtectionCompleteUnlessOpen", "NSURLFileProtectionCompleteUntilFirstUserAuthentication"]
        func bounded(_ value: String?) -> String {
            guard let value else { return "missing" }
            return known.contains(value) ? value : "unrecognized"
        }
        var attributes: [FileAttributeKey: Any]?
        do {
            let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
            attributes = attrs
            let raw = (attrs[.protectionKey] as? FileProtectionType)?.rawValue
                ?? (attrs[.protectionKey] as? String)
            let value = attrs[.protectionKey]
            let representation = value == nil ? "missing"
                : (value is FileProtectionType ? "FileProtectionType" : (value is String ? "String" : "other"))
            print("VCP_PROTECTION_PROBE label=\(label) api=FileManager exists=true present=\(value != nil) type=\(representation) value=\(bounded(raw))")
        } catch { protectionProbeError(error, label: label, operation: "FileManager.read") }
        do {
            var fresh = URL(fileURLWithPath: url.path)
            fresh.removeAllCachedResourceValues()
            let values = try fresh.resourceValues(forKeys: [.fileProtectionKey])
            print("VCP_PROTECTION_PROBE label=\(label) api=URL value=\(bounded(values.fileProtection?.rawValue))")
        } catch { protectionProbeError(error, label: label, operation: "URL.read") }
        return attributes
    }
    private func protectionProbeError(_ error: Error, label: String, operation: String) {
        let error = error as NSError
        let domain = [NSCocoaErrorDomain, NSPOSIXErrorDomain, NSOSStatusErrorDomain].contains(error.domain)
            ? error.domain : "other"
        print("VCP_PROTECTION_PROBE label=\(label) operation=\(operation) errorDomain=\(domain) code=\(error.code)")
    }
    private func diagnosePendingProtection(_ directory: URL, expected: NativePendingInfo) {
        #if targetEnvironment(simulator)
        print("VCP_PROTECTION_PROBE simulator=true")
        #else
        print("VCP_PROTECTION_PROBE simulator=false")
        #endif
        let main = directory.appendingPathComponent("pending.sqlite")
        let before = protectionProbe(main, label: "main.before-extra-open")
        _ = protectionProbe(directory, label: "root")
        do {
            let held = try NativePendingJournal.open(origin: a, privatePath: main.path)
            XCTAssertEqual(try held.pendingForTopic(topicKey: "topic_a"), [expected],
                "Diagnostic path must read the exact original journal")
            withExtendedLifetime(held) {
                for suffix in ["", "-wal", "-shm"] {
                    _ = protectionProbe(URL(fileURLWithPath: main.path + suffix), label: "held" + (suffix.isEmpty ? ".main" : suffix))
                }
            }
        } catch { protectionProbeError(error, label: "journal", operation: "open-or-read") }
        let after = protectionProbe(main, label: "main.after-extra-open")
        if let firstDevice = before?[.systemNumber] as? NSNumber,
           let firstInode = before?[.systemFileNumber] as? NSNumber,
           let lastDevice = after?[.systemNumber] as? NSNumber,
           let lastInode = after?[.systemFileNumber] as? NSNumber {
            print("VCP_PROTECTION_PROBE mainIdentityKnown=true same=\(firstDevice == lastDevice && firstInode == lastInode)")
        } else { print("VCP_PROTECTION_PROBE mainIdentityKnown=false") }
        // UUID-scoped synthetic control directory; no attributes of the actual
        // journal are changed, so the original assertion cannot be repaired by this probe.
        let controls = directory.appendingPathComponent("protection-probe", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: controls) }
        var operation = "createDirectory"
        do {
            try FileManager.default.createDirectory(at: controls, withIntermediateDirectories: false,
                attributes: [.protectionKey: FileProtectionType.complete])
            operation = "root.setComplete"
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: controls.path)
            print("VCP_PROTECTION_PROBE label=control.root operation=setComplete returned=true")
            _ = protectionProbe(controls, label: "control.root")
            let setFile = controls.appendingPathComponent("set-complete.bin")
            operation = "control.set.write"
            try Data([0]).write(to: setFile)
            _ = protectionProbe(setFile, label: "control.plain")
            operation = "control.set.setComplete"
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: setFile.path)
            print("VCP_PROTECTION_PROBE label=control.set operation=setComplete returned=true")
            _ = protectionProbe(setFile, label: "control.set")
            let writeFile = controls.appendingPathComponent("write-complete.bin")
            operation = "control.write.writeComplete"
            try Data([0]).write(to: writeFile, options: [.completeFileProtection])
            print("VCP_PROTECTION_PROBE label=control.write operation=writeComplete returned=true")
            _ = protectionProbe(writeFile, label: "control.write")
        } catch { protectionProbeError(error, label: "controls", operation: operation) }
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
        diagnosePendingProtection(directory, expected: pending)
        let attrs = try FileManager.default.attributesOfItem(atPath:directory.appendingPathComponent("pending.sqlite").path)
        let protectionValue = attrs[.protectionKey]
        // NSDictionary values need not arrive as the Swift typed wrapper.
        // Normalize the representation, never the required protection class.
        let protection = (protectionValue as? FileProtectionType)
            ?? (protectionValue as? String).map { FileProtectionType(rawValue: $0) }
        let valueType = protectionValue.map { String(reflecting: type(of: $0)) } ?? "missing"
        let knownProtectionValues = ["NSFileProtectionNone", "NSFileProtectionComplete",
            "NSFileProtectionCompleteUnlessOpen", "NSFileProtectionCompleteUntilFirstUserAuthentication"]
        let observedValue = protection.flatMap { knownProtectionValues.contains($0.rawValue) ? $0.rawValue : nil }
            ?? (protectionValue == nil ? "missing" : "unrecognized")
        // Bounded OS metadata only: no path, payload, Cookie or unknown value.
        print("VCP_PENDING_PROTECTION present=\(protectionValue != nil) type=\(valueType) value=\(observedValue)")
        XCTAssertEqual(protection?.rawValue, FileProtectionType.complete.rawValue,
            "Complete protection must be present; attribute representation: \(valueType)")
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
