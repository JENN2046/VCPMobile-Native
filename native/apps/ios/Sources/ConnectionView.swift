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
    @Published var pendingRows: [NativePendingPreview] = []
    @Published var pendingStatus = "PENDING_IDLE"
    @Published var recoveryStatus = "RECOVERY_IDLE"
    @Published var recoveryActive = false
    private var recoveryFence: NativeRecoveryFence?
    private var recoveryTask: Task<Void, Never>?

    @Published var draftText = ""
    @Published var sendStatus = "SEND_IDLE"
    @Published var sendReviewRequired = false
    @Published var sendActive = false
    private var sendFence: NativeRecoveryFence?
    private var sendTask: Task<Void, Never>?
    var confirmationGeneration: UInt64 { viewGeneration }
    var validDraft: Bool { isMessageTextValid(text: draftText) }
    func cancelSend() {
        guard let fence = sendFence else { return }
        fence.cancel()
        sendFence = nil; sendTask?.cancel(); sendTask = nil
        viewGeneration &+= 1; sendActive = false; busy = false
        sendStatus = "SEND_CANCELLED_REVIEW_PENDING"
    }
    /// Shared composer, one protected capability pipeline; no second sender.
    func sendMessage() {
        guard !busy, !sendReviewRequired, validDraft, status == "SESSION_READY",
              let expected = session, let topic = selectedTopic, topics.contains(topic) else { return }
        let message = draftText
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        let generation = viewGeneration
        let fence: NativeRecoveryFence
        do { fence = try NativeRecoveryFence(expected: expected, topicKey: topic, viewGeneration: generation) }
        catch { sendStatus = "SEND_CONTEXT_REJECTED"; return }
        let requestId = UUID().uuidString
        sendFence = fence; sendActive = true; busy = true
        sendReviewRequired = true; sendStatus = "SEND_CLAIMING_ONCE"
        sendTask = Task {
            defer {
                if sendFence === fence {
                    sendFence = nil; sendTask = nil; sendActive = false
                    if generation == viewGeneration { busy = false }
                }
            }
            do {
                let result = try await vault.sendOriginalMessageOnce(target, expected: expected,
                    topicKey: topic, requestId: requestId, text: message, generation: generation, fence: fence)
                try Task.checkCancellation()
                guard sendFence === fence, generation == viewGeneration,
                      target == origin.trimmingCharacters(in: .whitespacesAndNewlines), selectedTopic == topic,
                      session?.entryId == expected.entryId, session?.sessionMarker == expected.sessionMarker else { return }
                draftText = ""; sendStatus = result.disposition
                pendingRows = []; pendingStatus = "PENDING_REVIEW_REQUIRED"
            } catch {
                if sendFence === fence, generation == viewGeneration {
                    sendStatus = "SEND_UNCERTAIN_REVIEW_PENDING"
                    pendingStatus = "PENDING_REVIEW_REQUIRED"
                }
            }
        }
    }

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
                if result == nil {
                    sendReviewRequired = false; pendingRows = []; pendingStatus = "PENDING_EMPTY"
                } else if case .some(.accepted(_, _)) = result {
                    sendReviewRequired = false; pendingRows = []; pendingStatus = "PENDING_EMPTY"
                }
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
        cancelSend()
        viewGeneration &+= 1
        if dropSession { session = nil }
        selectedTopic = nil
        historyRows = []; historyCursor = 0; historyHasMore = false
        historyStatus = "HISTORY_IDLE"
        pendingRows = []; pendingStatus = "PENDING_IDLE"
        recoveryStatus = "RECOVERY_IDLE"
        draftText = ""; sendStatus = "SEND_IDLE"; sendReviewRequired = false
    }
    func selectTopic(_ topic: String) {
        guard (!busy || recoveryActive || sendActive), status == "SESSION_READY", topics.contains(topic), session != nil else { return }
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


    /// Explicit read-only session-checked local inventory, never a recovery POST.
    func inspectPending() {
        guard !busy, status == "SESSION_READY", let expected = session,
              let topic = selectedTopic, topics.contains(topic) else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        let generation = viewGeneration
        busy = true; pendingRows = []; pendingStatus = "PENDING_LOADING"
        Task {
            defer { busy = false }
            do {
                let previews = try await vault.listPending(target, expected: expected, topicKey: topic)
                guard generation == viewGeneration,
                      target == origin.trimmingCharacters(in: .whitespacesAndNewlines),
                      selectedTopic == topic, session?.entryId == expected.entryId,
                      session?.sessionMarker == expected.sessionMarker else {
                    pendingStatus = "PENDING_STALE"; return
                }
                pendingRows = previews
                sendReviewRequired = !previews.isEmpty
                pendingStatus = previews.isEmpty ? "PENDING_EMPTY" : "PENDING_UNKNOWN_REVIEW"
            } catch {
                if generation == viewGeneration { pendingStatus = "PENDING_FAILED" }
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
    @State private var confirmingSend = false
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
                            }.disabled(model.busy && !model.recoveryActive && !model.sendActive)
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
                    Section("原始消息（仅手动首次发送）") {
                        TextEditor(text: $model.draftText).frame(minHeight: 90)
                            .accessibilityIdentifier("messageDraft")
                            .disabled(model.busy || model.sendReviewRequired)
                        Button("确认后发送一次") { confirmingSend = true }
                            .accessibilityIdentifier("sendOriginalMessage")
                            .disabled(model.busy || model.sendReviewRequired || !model.validDraft)
                        Text(model.sendStatus).accessibilityIdentifier("sendStatus")
                        if model.sendActive {
                            Button("停止本机等待") { model.cancelSend() }.accessibilityIdentifier("cancelSend")
                        }
                        Text("新消息最多4000 UTF-16单位；HTTP202不代表AA完成。取消本机等待不会撤回已领取的请求或取消远端任务。")
                            .font(.caption)
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
                        Divider()
                        Button("核对本机待确认清单") { model.inspectPending() }
                            .accessibilityIdentifier("inspectPending").disabled(model.busy)
                        Text(model.pendingStatus).accessibilityIdentifier("pendingStatus")
                        ForEach(model.pendingRows.indices, id: \.self) { index in
                            let row = model.pendingRows[index]
                            Text("\(row.kind) · \(row.state) · \(row.recoveryMode) · \(row.requestId)")
                        }
                        Text("仅列原请求标识，不显示内容；清单不是服务端收件或AA完成的证明。跨会话只允许查询。")
                            .font(.caption)
                    }
                }
                Section {
                    Button("移除本机登录", role: .destructive) { model.removeLocal() }
                        .accessibilityIdentifier("removeConnection").disabled(model.busy)
                    Text("只填写你信任的服务入口。配对码仅用于一次请求；不放入日志、聊天历史或普通设置。")
                        .font(.caption)
                }
            }.navigationTitle("连接 VCP")
                .onDisappear { model.cancelSend(); model.cancelRecovery(); confirmingSend = false }
                .onChange(of: model.confirmationGeneration) { _, _ in confirmingSend = false }
                .onChange(of: model.draftText) { _, _ in confirmingSend = false }
                .confirmationDialog("确认原始消息首次提交", isPresented: $confirmingSend, titleVisibility: .visible) {
                    Button("确认发送一次") { model.sendMessage() }
                    Button("取消", role: .cancel) {}
                } message: {
                    Text("先在本机保存并只提交一次。断网和HTTP202不表示完成；未知状态先核对原回执，不自动重发。")
                }
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
