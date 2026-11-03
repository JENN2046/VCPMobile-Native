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
    @Published private(set) var executionStates: [String] = []
    @Published private(set) var topicSendReady = false
    @Published var pendingRows: [NativePendingPreview] = []
    @Published var pendingStatus = "PENDING_IDLE"
    @Published var draftText = ""
    @Published var sendStatus = "SEND_IDLE"
    // A fresh view must deliberately inspect the current journal before send.
    @Published var sendReviewRequired = true
    @Published private(set) var sendActive = false
    var confirmationGeneration: UInt64 { viewGeneration }
    private var sendFence: NativeRecoveryFence?
    private var sendTask: Task<Void, Never>?
    @Published private(set) var receivingReplies = false
    @Published private(set) var receiveStatus = "RECEIVE_IDLE"
    private var receiveTask: Task<Void, Never>?
    // A token distinct from viewGeneration also fences a stopped/restarted poll.
    private var receiveToken: UUID?
    // Explicit one-page GETs are tracked separately from reply polling.
    // Cancellation alone cannot retract an already-completed response;
    // viewGeneration fences every state update after backgrounding.
    private var historyTask: Task<Void, Never>?
    private var pendingInventoryTask: Task<Void, Never>?
    private func cancelLocalReads() {
        let hadRead = historyTask != nil || pendingInventoryTask != nil
        historyTask?.cancel(); historyTask = nil
        pendingInventoryTask?.cancel(); pendingInventoryTask = nil
        if hadRead {
            busy = false
            pendingStatus = "PENDING_RECHECK_REQUIRED"
        }
    }
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
        // A recovery probe may discover an unknown original or fail. It must
        // revoke a previous empty-inventory clearance before any async work.
        sendReviewRequired = true
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

    /// Requires the current history page to assert both execution flags and
    /// no remaining pages. Authentication alone never enables submission.
    var canSendOriginal: Bool {
        guard !busy, !sendReviewRequired, status == "SESSION_READY",
              session != nil, let topic = selectedTopic, topics.contains(topic),
              topicSendReady, !historyHasMore,
              isMessageTextValid(text: draftText) else { return false }
        return true
    }
    func prepareSendConfirmation() -> NativeSendConfirmation? {
        guard canSendOriginal, let expected = session,
              let topic = selectedTopic else { return nil }
        return NativeSendConfirmation(origin: origin.trimmingCharacters(in: .whitespacesAndNewlines),
            entryId: expected.entryId, sessionMarker: expected.sessionMarker,
            topic: topic, viewGeneration: viewGeneration, text: draftText)
    }
    func confirmSend(_ captured: NativeSendConfirmation) {
        guard let current = prepareSendConfirmation(), current == captured else {
            sendStatus = "SEND_CONFIRMATION_STALE"
            return
        }
        // Do not let an old confirmation revive after a later empty recheck.
        viewGeneration &+= 1
        sendMessage()
    }

    func updateDraft(_ value: String) {
        if !busy && !sendReviewRequired && topicSendReady && !historyHasMore &&
            value.utf16.count <= Int(messageInputMaxUtf16()) { draftText = value }
    }

    func cancelSend() {
        guard let fence = sendFence else { return }
        fence.cancel() // Synchronous cancellation before any stale claim.
        sendTask?.cancel()
        sendFence = nil; sendTask = nil
        viewGeneration &+= 1
        sendActive = false; busy = false
        sendReviewRequired = true // Claimed intent may already be pending.
        sendStatus = "SEND_STOPPED_CHECK_ORIGINAL"
    }

    /// Invoked only after the platform confirmation dialogue, never on restore.
    /// Reuses one journal/claim/receipt implementation rather than the old
    /// unfenced NativePendingJournal.sendOriginalMessageOnce convenience ABI.
    func sendMessage() {
        guard canSendOriginal, let expected = session,
              let topic = selectedTopic else { return }
        let text = draftText
        guard isMessageTextValid(text: text) else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        let generation = viewGeneration
        let fence: NativeRecoveryFence
        do { fence = try NativeRecoveryFence(expected: expected, topicKey: topic,
                                             viewGeneration: generation) }
        catch { sendStatus = "SEND_CONTEXT_REJECTED"; return }
        let requestId = UUID().uuidString
        sendFence = fence; sendActive = true; busy = true; sendReviewRequired = true
        sendStatus = "VERIFYING_BEFORE_SEND"
        sendTask = Task {
            defer {
                if sendFence === fence {
                    sendFence = nil; sendTask = nil; sendActive = false; busy = false
                }
            }
            do {
                let result = try await vault.sendOriginalMessageOnce(target, expected: expected,
                    topicKey: topic, requestId: requestId, text: text,
                    generation: generation, fence: fence)
                try Task.checkCancellation()
                guard sendFence === fence, generation == viewGeneration,
                      target == origin.trimmingCharacters(in: .whitespacesAndNewlines),
                      selectedTopic == topic, session?.entryId == expected.entryId,
                      session?.sessionMarker == expected.sessionMarker else { return }
                sendStatus = result.disposition
                draftText = ""
                pendingRows = []; pendingStatus = "PENDING_REVIEW_REQUIRED"
            } catch {
                if sendFence === fence, generation == viewGeneration {
                    sendStatus = "UNKNOWN_CHECK_ORIGINAL"
                    pendingStatus = "PENDING_REVIEW_REQUIRED"
                }
            }
        }
    }

    /// Explicit, bounded, read-only reply reception over the authenticated
    /// history route. This is not token streaming or remote task cancellation.
    func receiveReplies() {
        guard !busy, status == "SESSION_READY", let expected = session,
              let topic = selectedTopic, topics.contains(topic) else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        let generation = viewGeneration
        let token = UUID()
        receiveToken = token; receivingReplies = true; busy = true
        topicSendReady = false
        receiveStatus = "RECEIVING_HISTORY"
        receiveTask = Task {
            defer {
                if receiveToken == token {
                    receiveToken = nil; receiveTask = nil
                    receivingReplies = false; busy = false
                }
            }
            do {
                for _ in 0..<120 {
                    try Task.checkCancellation()
                    guard sameReplyView(token, generation: generation, target: target,
                                        expected: expected, topic: topic) else { return }
                    let after = historyCursor
                    let page = try await vault.fetchHistory(target, expected: expected,
                        topicKey: topic, after: after)
                    try Task.checkCancellation()
                    guard sameReplyView(token, generation: generation, target: target,
                                        expected: expected, topic: topic) else { return }
                    guard page.after == after, page.topicKey == topic,
                          page.entryId == expected.entryId,
                          page.sessionMarker == expected.sessionMarker else {
                        throw NativeHistoryWindowError.staleRevision
                    }
                    let next = try NativeHistoryWindow.merge(historyRows, incoming: page.rows)
                    let nextExecutionStates = NativeExecutionLabels.project(page.executionsJson)
                    historyRows = next
                    executionStates = nextExecutionStates
                    historyCursor = page.nextCursor
                    historyHasMore = page.hasMore
                    topicSendReady = NativeTopicSendGuard.allowsSend(
                        topicStateJson: page.topicStateJson, hasMore: page.hasMore)
                    historyStatus = next.isEmpty ? "HISTORY_EMPTY" : "HISTORY_READY"
                    receiveStatus = "RECEIVING_HISTORY"
                    // 120 bounded reads; normal polling every 2 seconds.
                    // Fast pagination is still read-only and never submits.
                    try await Task.sleep(nanoseconds: page.hasMore ? 100_000_000 : 2_000_000_000)
                }
                if sameReplyView(token, generation: generation, target: target,
                                 expected: expected, topic: topic) {
                    receiveStatus = "RECEIVING_PAUSED"
                }
            } catch is CancellationError {
                if receiveToken == token { receiveStatus = "RECEIVING_STOPPED" }
            } catch {
                if sameReplyView(token, generation: generation, target: target,
                                 expected: expected, topic: topic) {
                    receiveStatus = "RECEIVE_FAILED_CHECK_CONTEXT"
                    topicSendReady = false
                }
            }
        }
    }
    private func sameReplyView(_ token: UUID, generation: UInt64, target: String,
                               expected: NativeSessionInfo, topic: String) -> Bool {
        receiveToken == token && generation == viewGeneration &&
            target == origin.trimmingCharacters(in: .whitespacesAndNewlines) &&
            selectedTopic == topic && session?.entryId == expected.entryId &&
            session?.sessionMarker == expected.sessionMarker
    }
    /// Foreground-only local HTTP reception. Interruption or screen lock
    /// revokes view send clearance, but never clears the durable pending record
    /// or claims that the remote Agent/AA execution was cancelled.
    func suspendForBackground() {
        // The invalidation must precede task cancellation; in-flight GETs
        // must not restore old Topic/history or empty-journal send grants.
        viewGeneration &+= 1
        cancelLocalReads()
        cancelSend()
        cancelRecovery()
        stopReceiving()
        sendReviewRequired = true
        topicSendReady = false
        if status == "SESSION_READY" && selectedTopic != nil {
            historyStatus = "HISTORY_RECHECK_REQUIRED"
        }
    }

    func stopReceiving() {
        guard receiveToken != nil else { return }
        // Cancel only local read activity. It cannot cancel a remote AA task.
        receiveToken = nil
        receiveTask?.cancel(); receiveTask = nil
        receivingReplies = false; busy = false
        topicSendReady = false
        receiveStatus = "RECEIVING_STOPPED"
    }

    /// Ephemeral view-only cache. Does not alter protected credentials or pending.
    private func clearHistory(dropSession: Bool = true) {
        viewGeneration &+= 1
        cancelLocalReads()
        cancelRecovery()
        cancelSend()
        stopReceiving()
        if dropSession { session = nil }
        selectedTopic = nil
        historyRows = []; historyCursor = 0; historyHasMore = false
        executionStates = []
        historyStatus = "HISTORY_IDLE"; topicSendReady = false
        pendingRows = []; pendingStatus = "PENDING_IDLE"
        draftText = ""; sendStatus = "SEND_IDLE"; sendReviewRequired = true
        recoveryStatus = "RECOVERY_IDLE"
        receiveStatus = "RECEIVE_IDLE"
    }
    func selectTopic(_ topic: String) {
        guard (!busy || recoveryActive || sendActive || receivingReplies), status == "SESSION_READY", topics.contains(topic), session != nil else { return }
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
        busy = true; historyStatus = "HISTORY_LOADING"; topicSendReady = false
        historyTask = Task {
            defer {
                if generation == viewGeneration { historyTask = nil; busy = false }
            }
            do {
                let page = try await vault.fetchHistory(target, expected: expected, topicKey: topic, after: after)
                guard generation == viewGeneration, target == origin.trimmingCharacters(in: .whitespacesAndNewlines),
                      selectedTopic == topic, session?.entryId == expected.entryId,
                      session?.sessionMarker == expected.sessionMarker else {
                    if generation == viewGeneration { historyStatus = "HISTORY_STALE" }
                    return
                }
                guard page.after == after, page.topicKey == topic,
                      page.entryId == expected.entryId, page.sessionMarker == expected.sessionMarker else {
                    historyStatus = "HISTORY_REJECTED"
                    return
                }
                // Central history may revise or tombstone an existing ID.
                // Only strictly newer revisions can replace an earlier row.
                let next = try NativeHistoryWindow.merge(historyRows, incoming: page.rows)
                let nextExecutionStates = NativeExecutionLabels.project(page.executionsJson)
                historyRows = next
                executionStates = nextExecutionStates
                historyCursor = page.nextCursor
                historyHasMore = page.hasMore
                topicSendReady = NativeTopicSendGuard.allowsSend(
                    topicStateJson: page.topicStateJson, hasMore: page.hasMore)
                historyStatus = historyRows.isEmpty ? "HISTORY_EMPTY" : "HISTORY_READY"
                // Separate execution projection is not proof of AA completion.
            } catch NativeHistoryWindowError.staleRevision {
                if generation == viewGeneration { historyStatus = "HISTORY_REJECTED"; topicSendReady = false }
            } catch {
                if generation == viewGeneration { historyStatus = "HISTORY_FAILED"; topicSendReady = false }
            }
        }
    }


    /// Explicit read-only session-checked local inventory, never a recovery POST.
    func inspectPending() {
        guard !busy, status == "SESSION_READY", let expected = session,
              let topic = selectedTopic, topics.contains(topic) else { return }
        let target = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        let generation = viewGeneration
        // Fail closed when rechecking: a previously empty inventory is NOT
        // permission to send after a later failed/stale authenticated GET.
        sendReviewRequired = true
        busy = true; pendingRows = []; pendingStatus = "PENDING_LOADING"
        pendingInventoryTask = Task {
            defer {
                if generation == viewGeneration { pendingInventoryTask = nil; busy = false }
            }
            do {
                let previews = try await vault.listPending(target, expected: expected, topicKey: topic)
                guard generation == viewGeneration,
                      target == origin.trimmingCharacters(in: .whitespacesAndNewlines),
                      selectedTopic == topic, session?.entryId == expected.entryId,
                      session?.sessionMarker == expected.sessionMarker else {
                    if generation == viewGeneration { pendingStatus = "PENDING_STALE" }
                    return
                }
                pendingRows = previews
                pendingStatus = previews.isEmpty ? "PENDING_EMPTY" : "PENDING_UNKNOWN_REVIEW"
                sendReviewRequired = !previews.isEmpty
            } catch {
                if generation == viewGeneration {
                    sendReviewRequired = true
                    pendingStatus = "PENDING_FAILED"
                }
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
    @Environment(\.scenePhase) private var scenePhase
    @StateObject private var model = ConnectionModel()
    // Ephemeral only: never SceneStorage/AppStorage or a saved-state restoration field.
    @State private var code = ""
    @State private var confirmingSend: NativeSendConfirmation?
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
                            }.disabled(model.busy && !model.recoveryActive && !model.sendActive && !model.receivingReplies)
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
                        Text(model.receiveStatus).accessibilityIdentifier("receiveStatus")
                        Text("服务端任务状态快照（只读）").font(.headline)
                        ForEach(Array(model.executionStates.enumerated()), id: \.offset) { _, state in
                            Text(state)
                        }
                        Text("仅展示服务端状态字段，不等于原请求回执确认或 AA 整体完成。")
                            .font(.caption)
                        Button("开始接收回复（历史轮询）") { model.receiveReplies() }
                            .accessibilityIdentifier("receiveReplies").disabled(model.busy)
                        if model.receivingReplies {
                            Button("停止本机接收") { model.stopReceiving() }
                                .accessibilityIdentifier("stopReceiving")
                        }
                        Button("从头读取历史") { model.readHistory(restart: true) }
                            .accessibilityIdentifier("readHistory").disabled(model.busy)
                        ForEach(model.historyRows, id: \.sequence) { row in
                            Text("\(row.sequence) · \(historyPreview(row))")
                        }
                        if model.historyHasMore {
                            Button("读取下一页") { model.readHistory() }
                                .accessibilityIdentifier("nextHistoryPage").disabled(model.busy)
                        }
                        Text("仅内存保留最近400条。接收回复最多连续120次只读查询；停止不会取消远端任务。任务回执独立核对，不因读取历史而宣布任务完成。")
                            .font(.caption)
                        Divider()
                        Text("原始消息（手动首次发送）").font(.headline)
                        TextEditor(text: Binding(get: { model.draftText },
                                                 set: { model.updateDraft($0) }))
                            .frame(minHeight: 90)
                            .accessibilityIdentifier("messageDraft")
                            .disabled(model.busy || model.sendReviewRequired ||
                                !model.topicSendReady || model.historyHasMore)
                        Button("确认后发送一次") { confirmingSend = model.prepareSendConfirmation() }
                            .accessibilityIdentifier("sendOriginalMessage")
                            .disabled(!model.canSendOriginal)
                        Text(model.sendStatus).accessibilityIdentifier("sendStatus")
                        if model.sendActive {
                            Button("停止本机等待") { model.cancelSend() }.accessibilityIdentifier("cancelSend")
                        }
                        Text("发送前请核对本机待确认清单。HTTP 202 不是 AA 完成；未知原请求不可自动重发。")
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
                .confirmationDialog("确认原始消息首次提交", isPresented: Binding(
                    get: { confirmingSend != nil },
                    set: { if !$0 { confirmingSend = nil } }),
                    titleVisibility: .visible) {
                    Button("确认发送一次") {
                        let captured = confirmingSend
                        confirmingSend = nil // Consume ticket before any side effect.
                        if let captured { model.confirmSend(captured) }
                    }
                    Button("取消", role: .cancel) { confirmingSend = nil }
                } message: {
                    Text("本机只提交一次。网络失败或 HTTP 202 都要先核对原请求，不允许换 ID 盲目重发。")
                }
                .onChange(of: model.confirmationGeneration) { _, _ in confirmingSend = nil }
                .onChange(of: model.draftText) { _, _ in confirmingSend = nil }
                .onDisappear { model.suspendForBackground() }
                .onChange(of: scenePhase) { _, phase in
                    if phase != .active {
                        confirmingSend = nil
                        inputFocused = false
                        model.suspendForBackground()
                    }
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
