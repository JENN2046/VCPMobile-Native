import SwiftUI
import Foundation

private final class UiListener: ProbeListener, @unchecked Sendable {
    private let receive: @Sendable (ProbeProgress) -> Void
    init(_ receive: @escaping @Sendable (ProbeProgress) -> Void) { self.receive = receive }
    func onProgress(progress: ProbeProgress) throws { receive(progress) }
}

@MainActor final class ProbeModel: ObservableObject {
    @Published var status = "IDLE"
    @Published var detail = "只验证原生界面与 Rust 调用，尚未连接 NativeSync。"
    @Published var progress: UInt32 = 0
    @Published var running = false
    private var probe: BridgeProbe?
    private var task: Task<Void, Never>?
    private var generation = 0

    func start(_ mode: String) {
        guard !running else { return }
        generation += 1
        let ticket = generation
        let operation = BridgeProbe()
        probe = operation
        running = true
        progress = 0
        status = "RUNNING"
        let request: ProbeRequest
        switch mode {
        case "invalid": request = ProbeRequest(label: "invalid", steps: 0, intervalMs: 0)
        case "slow": request = ProbeRequest(label: "cancel", steps: 128, intervalMs: 1000)
        default: request = ProbeRequest(label: "跨端验证", steps: 3, intervalMs: 20)
        }
        let listener = UiListener { [weak self] event in
            Task { @MainActor in
                guard let self, self.generation == ticket, self.running else { return }
                self.progress = event.completed
            }
        }
        task = Task { [weak self] in
            do {
                let result = try await operation.run(request: request, listener: listener)
                guard let self, self.generation == ticket else { return }
                self.progress = result.completed
                self.status = "SUCCESS"
                self.detail = "共享 Rust 核心返回：\(result.label) · \(result.completed)"
            } catch {
                guard let self, self.generation == ticket else { return }
                let message = String(describing: error)
                self.status = message.lowercased().contains("cancel") ? "CANCELLED" : "ERROR"
                self.detail = message
            }
            guard let self, self.generation == ticket else { return }
            self.running = false
            self.probe = nil
            self.task = nil
        }
    }
    func cancel() { probe?.cancel() }
}

struct ProbeView: View {
    @StateObject private var model = ProbeModel()
    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text("M1 · 共享核心验证").font(.title2)
                    Text("这是工程验证界面，不是已接通服务的聊天客户端。")
                    Text(coreInfo()).font(.caption)
                    Text(model.status).font(.title).accessibilityIdentifier("status")
                    Text("进度：\(model.progress)").accessibilityIdentifier("progress")
                    Text(model.detail).font(.body)
                    Button("运行成功路径") { model.start("success") }
                        .accessibilityIdentifier("success").disabled(model.running)
                    Button("验证错误返回") { model.start("invalid") }
                        .accessibilityIdentifier("invalid").disabled(model.running)
                    Button("运行可取消路径") { model.start("slow") }
                        .accessibilityIdentifier("slow").disabled(model.running)
                    Button("取消本地验证", role: .destructive) { model.cancel() }
                        .accessibilityIdentifier("cancel").disabled(!model.running)
                }.padding(24).frame(maxWidth: .infinity, alignment: .leading)
            }.navigationTitle("VCP 原生开发版")
        }.onDisappear { model.cancel() }
    }
}

@main struct VCPNativeApp: App {
    var body: some Scene { WindowGroup { ProbeView() } }
}
