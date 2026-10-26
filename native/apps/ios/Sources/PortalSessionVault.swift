import Foundation
import Security

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
}
