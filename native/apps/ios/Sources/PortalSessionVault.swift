import Foundation
import Security
import CryptoKit

/// Never expose Keychain status descriptions, response bodies, or Cookie values.
enum PortalVaultError: Error {
    case unavailable, invalidRecord, notPaired, sessionChanged
}

/// App-private, device-only Keychain. An application should keep one actor per
/// service; the epoch only guards in-flight requests within this actor.
actor PortalSessionVault {
    private let service: String
    private var epochs: [String: UInt64] = [:]
    init(service: String = "org.vcptoolbox.nativeclient.dev.sessions.v1") {
        self.service = service
    }
    private func query(_ key: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: key,
         kSecAttrSynchronizable as String: false]
    }
    func save(_ credential: SessionCredential) throws {
        let key = try credential.storageKey()
        var data = try credential.secretBytesForVault()
        defer { data.resetBytes(in: 0..<data.count) }
        guard !data.isEmpty, data.count <= 2048 else { throw PortalVaultError.invalidRecord }
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        let base = query(key)
        var status = SecItemUpdate(base as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            let insert = base.merging(attributes) { _, new in new }
            status = SecItemAdd(insert as CFDictionary, nil)
            if status == errSecDuplicateItem {
                status = SecItemUpdate(base as CFDictionary, attributes as CFDictionary)
            }
        }
        guard status == errSecSuccess else { throw PortalVaultError.unavailable }
        epochs[key, default: 0] &+= 1
    }
    func load(_ origin: String) throws -> SessionCredential? {
        let key = try credentialStorageKey(origin: origin)
        var read = query(key)
        read[kSecReturnData as String] = true
        read[kSecMatchLimit as String] = kSecMatchLimitOne
        read[kSecUseAuthenticationUI as String] = kSecUseAuthenticationUIFail
        var result: CFTypeRef?
        let status = SecItemCopyMatching(read as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess else { throw PortalVaultError.unavailable }
        guard var data = result as? Data, !data.isEmpty, data.count <= 2048 else {
            throw PortalVaultError.invalidRecord
        }
        defer { data.resetBytes(in: 0..<data.count) }
        return try SessionCredential.restore(origin: origin, secretBytes: data)
    }
    /// Local removal only, never a remote logout or pending-message cleanup.
    func remove(_ origin: String) throws {
        let key = try credentialStorageKey(origin: origin)
        epochs[key, default: 0] &+= 1
        let status = SecItemDelete(query(key) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw PortalVaultError.unavailable
        }
    }
    /// One deliberate pairing POST. Persist before any session query.
    /// Removing/replacing the session while HTTPS is in flight fences this result.
    func pairAndSave(_ origin: String, code: String) async throws {
        let operation = try PairingAttempt(origin: origin, code: code)
        let key = try credentialStorageKey(origin: origin)
        epochs[key, default: 0] &+= 1
        let epoch = epochs[key, default: 0]
        let credential = try await operation.pair()
        try Task.checkCancellation()
        guard epochs[key, default: 0] == epoch else { throw PortalVaultError.sessionChanged }
        try save(credential)
    }
    /// Explicit read-only session query. Nothing runs on app launch.
    func fetchSession(_ origin: String) async throws -> NativeSessionInfo {
        let key = try credentialStorageKey(origin: origin)
        guard let credential = try load(origin) else { throw PortalVaultError.notPaired }
        let epoch = epochs[key, default: 0]
        let info = try await credential.fetchSessionInfo()
        guard epochs[key, default: 0] == epoch else { throw PortalVaultError.sessionChanged }
        return info
    }
    /// Explicit read-only history; credential replacement/removal invalidates
    /// the returned page. UI must additionally fence its selected Topic/view.
    func fetchHistory(_ origin: String, expected: NativeSessionInfo,
                      topicKey: String, after: UInt64) async throws -> NativeHistoryPage {
        let key = try credentialStorageKey(origin: origin)
        guard let credential = try load(origin) else { throw PortalVaultError.notPaired }
        let epoch = epochs[key, default: 0]
        let page = try await credential.fetchHistoryPage(expected: expected, topicKey: topicKey, after: after)
        try Task.checkCancellation()
        guard epochs[key, default: 0] == epoch else { throw PortalVaultError.sessionChanged }
        return page
    }


    /// A deliberate read-only local pending inventory; protected credential is
    /// checked against a fresh authenticated server Session. No POST or deletion.
    func listPending(_ origin: String, expected: NativeSessionInfo,
                     topicKey: String) async throws -> [NativePendingPreview] {
        let key = try credentialStorageKey(origin: origin)
        guard let credential = try load(origin) else { throw PortalVaultError.notPaired }
        let epoch = epochs[key, default: 0]
        let journal = try pendingJournal(origin)
        let previews = try await journal.listForTopic(
            credential: credential, expected: expected, topicKey: topicKey)
        try Task.checkCancellation()
        guard epochs[key, default: 0] == epoch else { throw PortalVaultError.sessionChanged }
        return previews
    }
    /// Complete file protection and backup exclusion are inherited by the
    /// SQLite/WAL files. No Cookie is written here; this is OS file protection,
    /// not a claim of application-level SQL encryption or physical erasure.
    private func pendingJournal(_ origin: String) throws -> NativePendingJournal {
        let key = try credentialStorageKey(origin: origin)
        let namespace = SHA256.hash(data: Data(service.utf8)).map { String(format: "%02x", $0) }.joined()
        let support = try FileManager.default.url(for: .applicationSupportDirectory,
            in: .userDomainMask, appropriateFor: nil, create: true)
        var root = support.appendingPathComponent("portal-pending-v1", isDirectory: true)
            .appendingPathComponent(namespace, isDirectory: true).appendingPathComponent(key, isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true,
            attributes: [.protectionKey: FileProtectionType.complete])
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: root.path)
        var values = URLResourceValues(); values.isExcludedFromBackup = true
        try root.setResourceValues(values)
        let previous = root.appendingPathComponent("pending.sqlite")
        let shared = support.appendingPathComponent("portal-pending-v1", isDirectory: true)
            .appendingPathComponent("vcp-pending-v1-\(key).sqlite")
        let hasShared = FileManager.default.fileExists(atPath: shared.path)
            || (try? FileManager.default.destinationOfSymbolicLink(atPath: shared.path)) != nil
        let hasPrevious = FileManager.default.fileExists(atPath: previous.path)
            || (try? FileManager.default.destinationOfSymbolicLink(atPath: previous.path)) != nil
        if hasShared && hasPrevious {
            throw PortalVaultError.invalidRecord
        }
        let path = hasShared ? shared.path : previous.path
        if hasShared {
            var parent = shared.deletingLastPathComponent()
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: parent.path)
            try parent.setResourceValues(values)
        }
        let journal = try NativePendingJournal.open(origin: origin, privatePath: path)
        for suffix in ["", "-wal", "-shm"] where FileManager.default.fileExists(atPath: path + suffix) {
            try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: path + suffix)
        }
        return journal
    }
    /// Storage checkpoint only. No POST, new execution or retry authority.
    func checkpointOriginal(_ origin: String, expected: NativeSessionInfo, topic: String,
                            kind: NativePendingKind, originalJson: String) throws -> NativePendingInfo {
        guard try load(origin) != nil else { throw PortalVaultError.notPaired }
        return try pendingJournal(origin).prepareOriginal(expected: expected, topicKey: topic, kind: kind, originalJson: originalJson)
    }
    func pendingOriginals(_ origin: String, topic: String) throws -> [NativePendingInfo] {
        try pendingJournal(origin).pendingForTopic(topicKey: topic)
    }
    /// Actor epoch, protected credential reload and synchronous token consume
    /// have no suspension point between them. Rust additionally serializes
    /// view cancellation with cleanup through the fence's lock.
    func recoverPending(_ origin: String, expected: NativeSessionInfo, topic: String,
                        generation: UInt64, fence: NativeRecoveryFence) async throws -> NativeRecoveryOutcome? {
        let key = try credentialStorageKey(origin: origin)
        guard let credential = try load(origin) else { throw PortalVaultError.notPaired }
        let epoch = epochs[key, default: 0]
        let journal = try pendingJournal(origin)
        let originals = try journal.pendingForTopic(topicKey: topic).filter { $0.entryId == expected.entryId }
        guard originals.count <= 1 else { throw PortalVaultError.invalidRecord }
        guard let original = originals.first else { return nil }
        let check = try await credential.queryPendingRecovery(journal: journal, pending: original, fence: fence)
        try Task.checkCancellation()
        guard epochs[key, default: 0] == epoch else { throw PortalVaultError.sessionChanged }
        guard let current = try load(origin) else { throw PortalVaultError.notPaired }
        return try current.confirmPendingRecovery(journal: journal, check: check, expected: expected,
            topicKey: topic, viewGeneration: generation)
    }

}
