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
    }
}
