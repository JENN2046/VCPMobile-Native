import SwiftUI
import Foundation

@MainActor final class ConnectionModel: ObservableObject {
    @Published var origin = UserDefaults.standard.string(forKey: "portal.origin.v1") ?? "" {
        didSet {
            if origin != oldValue {
                clearHistory()
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
    private var session: NativeSessionInfo?
    private var viewGeneration: UInt64 = 0
    @Published var selectedTopic: String?
    @Published var historyRows: [NativeHistoryRow] = []
    @Published var historyCursor: UInt64 = 0
    @Published var historyHasMore = false
    @Published var historyStatus = "HISTORY_IDLE"
    @Published var recoveryStatus = "RECOVERY_IDLE"
    @Published var recoveryActive = false
    private var recoveryFence: NativeRecoveryFence?
    private var recoveryTask: Task<Void, Never>?

    func cancelRecovery() {
        guard let fence = recoveryFence else { return }
        fence.cancel() // Synchronous barrier before changing view or Topic.
        recoveryFence = nil; recoveryActive = false
        recoveryTask?.cancel(); recoveryTask = nil
        viewGeneration &+= 1; busy = false; recoveryStatus = "RECOVERY_CANCELLED"
    }
    func recoverPending() {
        guard !busy, status == "SESSION_READY", let expected = session,
              let topic = selectedTopic, topics.contains(topic) else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        let generation = viewGeneration
        let fence: NativeRecoveryFence
        do { fence = try NativeRecoveryFence(expected: expected, topicKey: topic, viewGeneration: generation) }
        catch { recoveryStatus = "RECOVERY_REJECTED"; return }
        recoveryFence = fence; recoveryActive = true; busy = true; recoveryStatus = "RECOVERY_QUERYING"
        recoveryTask = Task {
            defer {
                if recoveryFence === fence {
                    recoveryFence = nil; recoveryActive = false; recoveryTask = nil; busy = false
                }
            }
            do {
                let result = try await vault.recoverPending(target, expected: expected, topic: topic, generation: generation, fence: fence)
                try Task.checkCancellation()
                guard recoveryFence === fence, generation == viewGeneration,
                      target == origin.trimmingCharacters(in: .whitespacesAndNewlines), selectedTopic == topic,
                      session?.entryId == expected.entryId, session?.sessionMarker == expected.sessionMarker else { return }
                switch result {
                case .none: recoveryStatus = "NO_PENDING_ORIGINAL"
                case .some(.accepted(_, let state)): recoveryStatus = "REQUEST_ACCEPTED · \(state.prefix(80)) · 非AA完成证明"
                case .some(.sameSessionNoReceipt): recoveryStatus = "NO_RECEIPT · 保留原请求，不自动重发"
                case .some(.review(_)): recoveryStatus = "REVIEW_REQUIRED · 保留原请求，不自动重发"
                }
            } catch {
                if recoveryFence === fence, generation == viewGeneration {
                    switch error {
                    case NativeRecoveryError.Expired, SessionCredentialError.Expired:
                        recoveryStatus = "RECOVERY_EXPIRED · 原请求保留，请恢复会话"
                    case NativeRecoveryError.AuthenticationRequired: recoveryStatus = "RECOVERY_AUTH_REQUIRED · 原请求保留"
                    case NativeRecoveryError.Forbidden: recoveryStatus = "RECOVERY_FORBIDDEN · 原请求保留"
                    case NativeRecoveryError.ContextChanged, NativeRecoveryError.CredentialRejected:
                        recoveryStatus = "RECOVERY_CONTEXT_CHANGED · 原请求保留"
                    case NativeRecoveryError.Cancelled: recoveryStatus = "RECOVERY_CANCELLED"
                    case PortalVaultError.notPaired: recoveryStatus = "RECOVERY_NOT_PAIRED · 原请求保留"
                    case PortalVaultError.sessionChanged: recoveryStatus = "RECOVERY_CONTEXT_CHANGED · 原请求保留"
                    default: recoveryStatus = "RECOVERY_FAILED · 原请求保留"
                    }
                }
            }
        }
    }

    /// Ephemeral view-only cache. Does not alter protected credentials or pending.
    private func clearHistory(dropSession: Bool = true) {
        cancelRecovery()
        viewGeneration &+= 1
        if dropSession { session = nil }
        selectedTopic = nil
        historyRows = []; historyCursor = 0; historyHasMore = false
        historyStatus = "HISTORY_IDLE"
        recoveryStatus = "RECOVERY_IDLE"
    }
    func selectTopic(_ topic: String) {
        guard (!busy || recoveryActive), status == "SESSION_READY", topics.contains(topic), session != nil else { return }
        clearHistory(dropSession: false)
        selectedTopic = topic
        historyStatus = "HISTORY_READY_TO_READ"
    }
    /// No automatic networking. One authorized GET page per explicit user action.
    func readHistory(restart: Bool = false) {
        guard !busy, status == "SESSION_READY", let expected = session,
              let topic = selectedTopic, topics.contains(topic) else { return }
        if restart {
            clearHistory(dropSession: false)
            selectedTopic = topic
        } else if !historyRows.isEmpty && !historyHasMore { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        let after = historyCursor
        let generation = viewGeneration
        busy = true; historyStatus = "HISTORY_LOADING"
        Task {
            defer { busy = false }
            do {
                let page = try await vault.fetchHistory(target, expected: expected, topicKey: topic, after: after)
                guard generation == viewGeneration, target == origin.trimmingCharacters(in: .whitespacesAndNewlines),
                      selectedTopic == topic, session?.entryId == expected.entryId,
                      session?.sessionMarker == expected.sessionMarker else {
                    historyStatus = "HISTORY_STALE"
                    return
                }
                guard page.after == after, page.topicKey == topic,
                      page.entryId == expected.entryId, page.sessionMarker == expected.sessionMarker else {
                    historyStatus = "HISTORY_REJECTED"
                    return
                }
                let seen = Set(historyRows.map(\.messageId))
                guard !page.rows.contains(where: { seen.contains($0.messageId) }) else {
                    historyStatus = "HISTORY_REJECTED"
                    return
                }
                historyRows = Array((historyRows + page.rows).suffix(400))
                historyCursor = page.nextCursor
                historyHasMore = page.hasMore
                historyStatus = historyRows.isEmpty ? "HISTORY_EMPTY" : "HISTORY_READY"
                // Separate execution projection is not proof of AA completion.
            } catch {
                if generation == viewGeneration { historyStatus = "HISTORY_FAILED" }
            }
        }
    }

    func pair(_ code: String) {
        guard !busy else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        busy = true; clearHistory(); topics = []; status = "PAIRING"
        Task {
            var saved = false
            defer { busy = false }
            do {
                try await vault.pairAndSave(target, code: code)
                saved = true
                UserDefaults.standard.set(target, forKey: "portal.origin.v1")
                status = "SAVED_SESSION_UNCONFIRMED"
                let info = try await vault.fetchSession(target)
                session = info; topics = info.topicKeys; status = "SESSION_READY"
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
        busy = true; clearHistory(); topics = []; status = "RESTORING"
        Task {
            defer { busy = false }
            do {
                let info = try await vault.fetchSession(target)
                UserDefaults.standard.set(target, forKey: "portal.origin.v1")
                session = info; topics = info.topicKeys; status = "SESSION_READY"
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
        busy = true; clearHistory(); topics = []
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
                    Section("授权话题") {
                        ForEach(model.topics, id: \.self) { topic in
                            Button(model.selectedTopic == topic ? "✓ \(topic)" : topic) {
                                model.selectTopic(topic)
                            }.disabled(model.busy && !model.recoveryActive)
                        }
                    }
                }
                if model.selectedTopic != nil {
                    Section("待确认提交") {
                        Button("核对待确认提交（只查询）") { model.recoverPending() }
                            .accessibilityIdentifier("recoverPending").disabled(model.busy)
                        Text(model.recoveryStatus).accessibilityIdentifier("recoveryStatus")
                        if model.recoveryActive {
                            Button("取消核对") { model.cancelRecovery() }.accessibilityIdentifier("cancelRecovery")
                        }
                    }
                    Section("话题历史（只读）") {
                        Text(model.historyStatus).accessibilityIdentifier("historyStatus")
                        Button("从头读取历史") { model.readHistory(restart: true) }
                            .accessibilityIdentifier("readHistory").disabled(model.busy)
                        ForEach(model.historyRows, id: \.sequence) { row in
                            Text("\(row.sequence) · \(historyPreview(row))")
                        }
                        if model.historyHasMore {
                            Button("读取下一页") { model.readHistory() }
                                .accessibilityIdentifier("nextHistoryPage").disabled(model.busy)
                        }
                        Text("仅内存保留最近400条。任务回执独立核对，不因读取历史而宣布任务完成。")
                            .font(.caption)
                    }
                }
                Section {
                    Button("移除本机登录", role: .destructive) { model.removeLocal() }
                        .accessibilityIdentifier("removeConnection").disabled(model.busy)
                    Text("只填写你信任的服务入口。配对码仅用于一次请求；不放入日志、聊天历史或普通设置。")
                        .font(.caption)
                }
            }.navigationTitle("连接 VCP").onDisappear { model.cancelRecovery() }
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

/// Plain SwiftUI Text: never parse untrusted HTML or execute message content.
private func historyPreview(_ row: NativeHistoryRow) -> String {
    if row.deleted { return "[消息已删除]" }
    guard let raw = row.messageJson, let data = raw.data(using: .utf8),
          let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
        return "[消息无法预览]"
    }
    let role = String((object["role"] as? String ?? "消息").prefix(24))
    let content = (object["content"] as? String).map { String($0.prefix(400)) } ?? "[非纯文本内容]"
    return "\(role): \(content)"
}
