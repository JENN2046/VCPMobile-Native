import Foundation

final class Recorder: ProbeListener, @unchecked Sendable {
    private let lock = NSLock()
    private var values: [UInt32] = []
    private let cancel: (() -> Void)?
    init(cancel: (() -> Void)? = nil) { self.cancel = cancel }
    func onProgress(progress: ProbeProgress) throws {
        lock.lock()
        values.append(progress.completed)
        lock.unlock()
        cancel?()
    }
    func events() -> [UInt32] {
        lock.lock()
        defer { lock.unlock() }
        return values
    }
}

enum ContractFailure: Error { case assertion(String) }
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw ContractFailure.assertion(message) }
}

@main struct BindingContract {
    static func main() async throws {
        print(coreInfo())
        let probe = BridgeProbe()
        let listener = Recorder()
        let result = try await probe.run(
            request: ProbeRequest(label: "跨端 probe 🧭", steps: 3, intervalMs: 0),
            listener: listener)
        try require(result.label == "跨端 probe 🧭", "Unicode round trip")
        try require(result.completed == 3, "result count")
        try require(listener.events() == [1, 2, 3], "ordered callbacks")
        print("PASS: Swift success and callbacks")

        var invalidRejected = false
        do {
            _ = try await BridgeProbe().run(
                request: ProbeRequest(label: "invalid", steps: 0, intervalMs: 0),
                listener: Recorder())
        } catch { invalidRejected = String(describing: error).contains("steps") }
        try require(invalidRejected, "invalid request must return structured Rust error")
        print("PASS: Swift invalid input")

        let cancelled = BridgeProbe()
        let cancellationListener = Recorder { cancelled.cancel() }
        var wasCancelled = false
        do {
            _ = try await cancelled.run(
                request: ProbeRequest(label: "cancel", steps: 10, intervalMs: 1),
                listener: cancellationListener)
        } catch { wasCancelled = String(describing: error).lowercased().contains("cancel") }
        try require(wasCancelled, "cancel must cross the binding")
        try require(cancellationListener.events() == [1], "no callback after observed cancel")
        print("PASS: Swift reentrant cancellation")

        var duplicateRejected = false
        do {
            _ = try await probe.run(
                request: ProbeRequest(label: "duplicate", steps: 1, intervalMs: 0),
                listener: Recorder())
        } catch { duplicateRejected = true }
        try require(duplicateRejected, "completed probe cannot restart")
        print("PASS: Swift one-shot lifecycle")

        try require(messageInputMaxUtf16() == 4000,"shared UTF16 limit")
        for text in [String(repeating:"中",count:4000),String(repeating:"🧭",count:2000),String(repeating:"中",count:3998)+"🧭"] {
            try require(isMessageTextValid(text:text),"UTF16 boundary accepted")
        }
        for text in [String(repeating:"中",count:4001),String(repeating:"🧭",count:2001),String(repeating:"中",count:3999)+"🧭"] {
            try require(!isMessageTextValid(text:text),"UTF16 boundary refused")
        }
        print("PASS: Swift shared UTF16 policy")

        let root = FileManager.default.temporaryDirectory.appendingPathComponent("vcp-native-recovery-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at:root,withIntermediateDirectories:true)
        defer { try? FileManager.default.removeItem(at:root) }
        let origin = "https://recovery-binding.example.test"
        let expected = NativeSessionInfo(entryId:String(repeating:"a",count:64),sessionMarker:String(repeating:"b",count:64),
            topicKeys:["topic_a"],selectedTopic:nil)
        var journal: NativePendingJournal? = try NativePendingJournal.openScoped(origin:origin,privateDirectory:root.path)
        let original = try journal!.prepareOriginal(expected:expected,topicKey:"topic_a",kind:.message,
            originalJson:"{ \"requestId\":\"binding_pending_123\", \"text\":\"synthetic\" }")
        journal = nil
        let reopened = try NativePendingJournal.openScoped(origin:origin,privateDirectory:root.path)
        let credential = try SessionCredential.fromServerCookie(origin:origin,responseOrigin:origin,
            setCookie:"__Secure-vcp_mobile=\(String(repeating:"B",count:43)); Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600")
        let fence = try NativeRecoveryFence(expected:expected,topicKey:"topic_a",viewGeneration:3)
        try require(try reopened.pendingForTopic(topicKey:"topic_a") == [original],"checkpoint reopens across native handles")
        fence.cancel()
        var recoveryCancelled = false
        do { _ = try await credential.queryPendingRecovery(journal:reopened,pending:original,fence:fence) }
        catch NativeRecoveryError.Cancelled { recoveryCancelled = true }
        try require(recoveryCancelled,"cancelled recovery must not reach HTTPS")
        try require(try reopened.pendingForTopic(topicKey:"topic_a") == [original],"cancel retains original")
        print("PASS: Swift pending reopen and cancelled recovery")
        var deniedTopic = false
        do { _ = try NativeRecoveryFence(expected:expected,topicKey:"topic_other",viewGeneration:1) }
        catch NativeRecoveryError.Forbidden { deniedTopic = true }
        try require(deniedTopic,"unauthorized recovery Topic is rejected locally")
        print("PASS: Swift unauthorized recovery Topic")
    }
}
