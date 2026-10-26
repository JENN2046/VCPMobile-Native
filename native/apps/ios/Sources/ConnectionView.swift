import SwiftUI
import Foundation

@MainActor final class ConnectionModel: ObservableObject {
    @Published var origin = UserDefaults.standard.string(forKey: "portal.origin.v1") ?? "" {
        didSet {
            if origin != oldValue {
                topics = []; status = "NOT_CONNECTED"
                detail = "入口已改变，请配对或核对该入口的已保存会话。"
            }
        }
    }
    @Published var status = "NOT_CONNECTED"
    @Published var detail = "输入受信任 VCP 的 HTTPS 入口和新配对码。启动不会自动联网。"
    @Published var topics: [String] = []
    @Published var busy = false
    private let vault = PortalSessionVault()

    func pair(_ code: String) {
        guard !busy else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        busy = true; topics = []; status = "PAIRING"
        Task {
            var saved = false
            defer { busy = false }
            do {
                try await vault.pairAndSave(target, code: code)
                saved = true
                UserDefaults.standard.set(target, forKey: "portal.origin.v1")
                status = "SAVED_SESSION_UNCONFIRMED"
                let info = try await vault.fetchSession(target)
                topics = info.topicKeys; status = "SESSION_READY"
                detail = "登录已安全保存，授权话题已读取。当前不发送聊天或任务。"
            } catch {
                status = saved ? "SAVED_SESSION_UNCONFIRMED" : "PAIR_FAILED"
                detail = saved ? "登录已保存，但会话查询失败。请点恢复会话，不要重发配对码。"
                    : "配对未完成或未安全保存。请检查入口和配对码；网络中断时配对码可能已被使用，不能自动重发。"
            }
        }
    }
    func restore() {
        guard !busy else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        busy = true; topics = []; status = "RESTORING"
        Task {
            defer { busy = false }
            do {
                let info = try await vault.fetchSession(target)
                UserDefaults.standard.set(target, forKey: "portal.origin.v1")
                topics = info.topicKeys; status = "SESSION_READY"
                detail = "已从安全存储恢复并向服务端核对会话。没有重复配对或提交任务。"
            } catch {
                status = "RESTORE_FAILED"
                detail = "会话尚未配对、已过期或暂时无法核对。原凭据保留；不会自动重新配对。"
            }
        }
    }
    func removeLocal() {
        guard !busy else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        busy = true; topics = []
        Task {
            defer { busy = false }
            do {
                try await vault.remove(target)
                status = "REMOVED_LOCAL"
                detail = "仅移除了此入口在本机的登录。不撤销远端会话、不取消任务、不删除待确认提交。"
            } catch { status = "REMOVE_FAILED"; detail = "本机凭据未能确认删除，请重试。" }
        }
    }
}

struct ConnectionView: View {
    @StateObject private var model = ConnectionModel()
    // Ephemeral only: never SceneStorage/AppStorage or a saved-state restoration field.
    @State private var code = ""
    @FocusState private var inputFocused: Bool
    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text("M2 · 配对与会话恢复；尚不是完整聊天客户端。")
                    TextField("HTTPS 入口，不含 /mobile 路径", text: $model.origin)
                        .keyboardType(.URL).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .accessibilityIdentifier("connectionOrigin").disabled(model.busy)
                    SecureField("一次性配对码", text: $code)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .accessibilityIdentifier("pairingCode").disabled(model.busy)
                    Button("配对并安全保存") {
                        inputFocused = false
                        let submitted = code; code = ""; model.pair(submitted)
                    }.accessibilityIdentifier("pairConnection").disabled(model.busy)
                    Button("恢复会话（只查询）") { inputFocused = false; model.restore() }
                        .accessibilityIdentifier("restoreConnection").disabled(model.busy)
                }.focused($inputFocused).submitLabel(.done).onSubmit { inputFocused = false }
                Section("连接状态") {
                    Text(model.status).accessibilityIdentifier("connectionStatus")
                    Text(model.detail)
                    if model.busy { ProgressView() }
                }
                if !model.topics.isEmpty {
                    Section("授权话题") { ForEach(model.topics, id: \.self) { Text($0) } }
                }
                Section {
                    Button("移除本机登录", role: .destructive) { model.removeLocal() }
                        .accessibilityIdentifier("removeConnection").disabled(model.busy)
                    Text("只填写你信任的服务入口。配对码仅用于一次请求；不放入日志、聊天历史或普通设置。")
                        .font(.caption)
                }
            }.navigationTitle("连接 VCP")
        }
    }
}

struct NativeRoot: View {
    var body: some View {
        TabView {
            ProbeView().tabItem { Label("工程验证", systemImage: "wrench.and.screwdriver") }
            ConnectionView().tabItem { Label("连接 VCP", systemImage: "network") }
        }
    }
}
