package org.vcptoolbox.nativeclient

import android.app.Application
import android.content.Context
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import java.util.UUID
import org.vcptoolbox.nativecore.messageInputMaxUtf16
import org.vcptoolbox.nativecore.isMessageTextValid
import org.vcptoolbox.nativecore.NativeSendException
import org.json.JSONObject
import org.vcptoolbox.nativecore.NativeHistoryRow
import org.vcptoolbox.nativecore.NativeSessionInfo
import org.vcptoolbox.nativecore.NativePendingPreview
import org.vcptoolbox.nativecore.NativeRecoveryFence
import org.vcptoolbox.nativecore.NativeRecoveryOutcome
import org.vcptoolbox.nativecore.NativeRecoveryException
import org.vcptoolbox.nativecore.SessionCredentialException

/** Only non-secret origin preference is persisted outside the protected vault. */
internal class ConnectionViewModel(app: Application) : AndroidViewModel(app) {
    private val prefs = app.getSharedPreferences("portal-origin-v1", Context.MODE_PRIVATE)
    private val vault = PortalSessionVault(app)
    var origin by mutableStateOf(prefs.getString("origin", "") ?: ""); private set
    var status by mutableStateOf("NOT_CONNECTED"); private set
    var detail by mutableStateOf("输入受信任 VCP 的 HTTPS 入口和新配对码。启动不会自动联网。"); private set
    var topics by mutableStateOf<List<String>>(emptyList()); private set
    var busy by mutableStateOf(false); private set
    private var session: NativeSessionInfo? = null
    private var viewGeneration = 0L
    var selectedTopic by mutableStateOf<String?>(null); private set
    var historyRows by mutableStateOf<List<NativeHistoryRow>>(emptyList()); private set
    var historyCursor by mutableStateOf(0uL); private set
    var historyHasMore by mutableStateOf(false); private set
    var historyStatus by mutableStateOf("HISTORY_IDLE"); private set
    var pendingRows by mutableStateOf<List<NativePendingPreview>>(emptyList()); private set
    var pendingStatus by mutableStateOf("PENDING_IDLE"); private set
    var recoveryStatus by mutableStateOf("RECOVERY_IDLE"); private set
    var recoveryActive by mutableStateOf(false); private set
    private var recoveryFence: NativeRecoveryFence? = null
    private var recoveryJob: Job? = null
    override fun onCleared() {
        recoveryFence?.cancel()
        chatFence?.cancel()
        chatJob?.cancel()
        super.onCleared()
    }

    fun cancelRecovery() {
        val fence = recoveryFence ?: return
        fence.cancel() // Linearizes before view invalidation or Topic change.
        recoveryFence = null; recoveryActive = false
        recoveryJob?.cancel(); recoveryJob = null
        viewGeneration++; busy = false; recoveryStatus = "RECOVERY_CANCELLED"
    }

    fun recoverPending() {
        if (busy || status != "SESSION_READY") return
        val expected = session ?: return
        val topic = selectedTopic ?: return
        if (topic !in topics) return
        val target = origin.trim()
        val generation = viewGeneration
        val fence = try { NativeRecoveryFence(expected, topic, generation.toULong()) }
            catch (_: Exception) { recoveryStatus = "RECOVERY_REJECTED"; return }
        recoveryFence = fence; recoveryActive = true; busy = true; recoveryStatus = "RECOVERY_QUERYING"
        recoveryJob = viewModelScope.launch {
            try {
                val result = vault.recoverPending(target, expected, topic, generation.toULong(), fence)
                if (recoveryFence !== fence || generation != viewGeneration || target != origin.trim() ||
                    selectedTopic != topic || session?.entryId != expected.entryId || session?.sessionMarker != expected.sessionMarker) return@launch
                if (result == null || result is NativeRecoveryOutcome.Accepted) {
                    unresolvedSend = false; pendingRows = emptyList(); pendingStatus = "PENDING_EMPTY"
                }
                recoveryStatus = when (result) {
                    null -> "NO_PENDING_ORIGINAL"
                    is NativeRecoveryOutcome.Accepted -> "REQUEST_ACCEPTED · ${result.executionState.take(80)} · 非AA完成证明"
                    is NativeRecoveryOutcome.SameSessionNoReceipt -> "NO_RECEIPT · 保留原请求，不自动重发"
                    is NativeRecoveryOutcome.Review -> "REVIEW_REQUIRED · 保留原请求，不自动重发"
                }
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (failure: Exception) {
                if (recoveryFence === fence && generation == viewGeneration) recoveryStatus = when (failure) {
                    is NativeRecoveryException.Expired, is SessionCredentialException.Expired -> "RECOVERY_EXPIRED · 原请求保留，请恢复会话"
                    is NativeRecoveryException.AuthenticationRequired -> "RECOVERY_AUTH_REQUIRED · 原请求保留"
                    is NativeRecoveryException.Forbidden -> "RECOVERY_FORBIDDEN · 原请求保留"
                    is NativeRecoveryException.ContextChanged, is NativeRecoveryException.CredentialRejected -> "RECOVERY_CONTEXT_CHANGED · 原请求保留"
                    is NativeRecoveryException.Cancelled -> "RECOVERY_CANCELLED"
                    is PortalVaultException -> if (failure.code == "NOT_PAIRED") "RECOVERY_NOT_PAIRED · 原请求保留" else "RECOVERY_CONTEXT_OR_STORAGE_REJECTED · 原请求保留"
                    else -> "RECOVERY_FAILED · 原请求保留"
                }
            } finally {
                if (recoveryFence === fence) {
                    recoveryFence = null; recoveryActive = false; recoveryJob = null; busy = false
                }
                fence.destroy()
            }
        }
    }

    var draft by mutableStateOf(""); private set
    var outgoingText by mutableStateOf(""); private set
    var sendStatus by mutableStateOf("SEND_IDLE"); private set
    var replyStatus by mutableStateOf("REPLY_IDLE"); private set
    var chatActive by mutableStateOf(false); private set
    private var unresolvedSend = false
    private var sendingOriginal = false
    private var historyExecutable = false
    private var chatFence: NativeRecoveryFence? = null
    private var chatJob: Job? = null
    val confirmationGeneration: Long get() = viewGeneration
    private val inputLimit = messageInputMaxUtf16().toInt()
    fun updateDraft(value: String) { if (!chatActive && value.length <= inputLimit) draft = value }
    val canSend: Boolean get() = !busy && status == "SESSION_READY" && selectedTopic != null &&
        historyExecutable && !historyHasMore && !unresolvedSend && pendingRows.isEmpty() && isMessageTextValid(draft)
    fun stopChat() {
        chatFence?.cancel()
        chatJob?.cancel()
        if (chatActive) {
            if (sendingOriginal) sendStatus = "STOPPED_CHECK_ORIGINAL"
            replyStatus = "RECEIVING_STOPPED"
            chatActive = false; busy = false
        }
    }
    private fun sameChatView(generation: Long, target: String, expected: NativeSessionInfo, topic: String): Boolean =
        generation == viewGeneration && target == origin.trim() && selectedTopic == topic &&
            session?.entryId == expected.entryId && session?.sessionMarker == expected.sessionMarker
    /** Explicit send, then bounded read-only updates through the existing
     * authorized history route. No invented SSE route or retry of a POST. */
    fun sendMessage() {
        if (!canSend) return
        startChat(sendText = draft)
    }
    fun receiveReplies() {
        if (busy || status != "SESSION_READY" || selectedTopic == null) return
        startChat(sendText = null)
    }
    private fun startChat(sendText: String?) {
        val expected = session ?: return
        val topic = selectedTopic ?: return
        if (topic !in topics) return
        val target = origin.trim(); val generation = viewGeneration
        val fence = try { NativeRecoveryFence(expected, topic, generation.toULong()) }
            catch (_: Exception) { sendStatus = "SEND_CONTEXT_REJECTED"; return }
        val requestId = if (sendText != null) UUID.randomUUID().toString() else null
        chatFence = fence; chatActive = true; busy = true
        sendingOriginal = sendText != null
        if (sendText != null) { unresolvedSend = true; outgoingText = sendText; draft = ""; sendStatus = "VERIFYING_BEFORE_SEND" }
        replyStatus = "RECEIVING"
        chatJob = viewModelScope.launch {
            try {
                if (sendText != null && requestId != null) {
                    val result = vault.sendMessage(target, expected, topic, generation.toULong(), requestId, sendText, fence)
                    if (!sameChatView(generation, target, expected, topic)) return@launch
                    sendStatus = result.disposition
                    // HTTP202 and any failed ACK remain UNKNOWN. Even a
                    // running receipt is not AA completion; recovery is separate.
                }
                repeat(120) {
                    if (!sameChatView(generation, target, expected, topic)) return@launch
                    val after = historyCursor
                    val page = vault.fetchHistory(target, expected, topic, after)
                    if (!sameChatView(generation, target, expected, topic)) return@launch
                    if (page.after != after || page.topicKey != topic || page.entryId != expected.entryId ||
                        page.sessionMarker != expected.sessionMarker) throw PortalVaultException("STALE_HISTORY")
                    historyRows = mergeHistoryWindow(historyRows, page.rows)
                    historyCursor = page.nextCursor; historyHasMore = page.hasMore
                    historyStatus = if (historyRows.isEmpty()) "HISTORY_EMPTY" else "HISTORY_READY"
                    historyExecutable = JSONObject(page.topicStateJson).optBoolean("historyReady", false) &&
                        JSONObject(page.topicStateJson).optBoolean("executable", false)
                    replyStatus = "RECEIVING"
                    delay(if (page.hasMore) 100L else 2000L)
                }
                if (sameChatView(generation, target, expected, topic)) replyStatus = "RECEIVING_PAUSED"
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (error: Exception) {
                if (sameChatView(generation, target, expected, topic)) {
                    replyStatus = "READ_FAILED_RECONNECT_AND_CHECK_ORIGINAL"
                    if (sendText != null) sendStatus = when (error) {
                        is NativeSendException.OutstandingPending -> "PENDING_EXISTS_CHECK_ORIGINAL"
                        is NativeSendException.InvalidMessage -> "INVALID_MESSAGE_NOT_SENT"
                        is NativeSendException.ContextChanged -> "CONTEXT_CHANGED_NOT_SENT"
                        is NativeSendException.TopicUnauthorized -> "TOPIC_DENIED_NOT_SENT"
                        else -> "UNKNOWN_CHECK_ORIGINAL"
                    }
                }
            } finally {
                if (chatFence === fence) {
                    chatFence = null; chatJob = null
                    chatActive = false; sendingOriginal = false
                    if (generation == viewGeneration) busy = false
                }
                fence.destroy()
            }
        }
    }

    /** Ephemeral view cache only. Never persist content in preferences or logs. */
    private fun clearHistory(dropSession: Boolean = true) {
        cancelRecovery()
        stopChat()
        viewGeneration++
        if (dropSession) session = null
        selectedTopic = null
        historyRows = emptyList(); historyCursor = 0uL; historyHasMore = false
        historyStatus = "HISTORY_IDLE"
        pendingRows = emptyList(); pendingStatus = "PENDING_IDLE"
        recoveryStatus = "RECOVERY_IDLE"
        draft = ""; outgoingText = ""; sendStatus = "SEND_IDLE"; replyStatus = "REPLY_IDLE"
        historyExecutable = false
        unresolvedSend = false; sendingOriginal = false
    }

    fun selectTopic(topic: String) {
        if ((busy && !recoveryActive && !chatActive) || status != "SESSION_READY" || topic !in topics || session == null) return
        clearHistory(dropSession = false)
        selectedTopic = topic
        historyStatus = "HISTORY_READY_TO_READ"
    }

    /** A deliberate one-page GET; no automatic pagination, POST or task control. */
    fun readHistory(restart: Boolean = false) {
        if (busy || status != "SESSION_READY") return
        val expected = session ?: return
        val topic = selectedTopic ?: return
        if (topic !in topics) return
        if (restart) {
            clearHistory(dropSession = false)
            selectedTopic = topic
        } else if (historyRows.isNotEmpty() && !historyHasMore) return
        val target = origin.trim()
        val after = historyCursor
        val generation = viewGeneration
        busy = true; historyStatus = "HISTORY_LOADING"
        viewModelScope.launch {
            try {
                val page = vault.fetchHistory(target, expected, topic, after)
                if (generation != viewGeneration || target != origin.trim() ||
                    selectedTopic != topic || session?.entryId != expected.entryId ||
                    session?.sessionMarker != expected.sessionMarker) {
                    return@launch
                }
                if (page.after != after || page.topicKey != topic ||
                    page.entryId != expected.entryId || page.sessionMarker != expected.sessionMarker ||
                    page.rows.any { row -> historyRows.any { it.messageId == row.messageId && it.sequence >= row.sequence } }) {
                    historyStatus = "HISTORY_REJECTED"
                    return@launch
                }
                historyRows = mergeHistoryWindow(historyRows, page.rows)
                historyExecutable = JSONObject(page.topicStateJson).optBoolean("historyReady", false) &&
                    JSONObject(page.topicStateJson).optBoolean("executable", false)
                historyCursor = page.nextCursor
                historyHasMore = page.hasMore
                historyStatus = if (historyRows.isEmpty()) "HISTORY_EMPTY" else "HISTORY_READY"
                // Execution receipts in page.executionsJson are NOT task completion.
                // Pending and AA status remain untouched by this read-only view.
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) {
                if (generation == viewGeneration) historyStatus = "HISTORY_FAILED"
            } finally { busy = false }
        }
    }


    /** User-triggered, read-only local inventory after live session verification. */
    fun inspectPending() {
        if (busy || status != "SESSION_READY") return
        val expected = session ?: return
        val topic = selectedTopic ?: return
        if (topic !in topics) return
        val target = origin.trim()
        val generation = viewGeneration
        busy = true; pendingRows = emptyList(); pendingStatus = "PENDING_LOADING"
        viewModelScope.launch {
            try {
                val previews = vault.listPending(target, expected, topic)
                if (generation != viewGeneration || target != origin.trim() ||
                    selectedTopic != topic || session?.entryId != expected.entryId ||
                    session?.sessionMarker != expected.sessionMarker) {
                    return@launch
                }
                pendingRows = previews
                unresolvedSend = previews.isNotEmpty()
                pendingStatus = if (previews.isEmpty()) "PENDING_EMPTY" else "PENDING_UNKNOWN_REVIEW"
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) {
                if (generation == viewGeneration) pendingStatus = "PENDING_FAILED"
            } finally { busy = false }
        }
    }

    fun updateOrigin(value: String) {
        if (busy) return
        clearHistory()
        origin = value; topics = emptyList(); status = "NOT_CONNECTED"
        detail = "入口已改变，请配对或核对该入口的已保存会话。"
    }
    fun pair(code: String) {
        if (busy) return
        val target = origin.trim()
        busy = true; clearHistory(); topics = emptyList(); status = "PAIRING"
        viewModelScope.launch {
            var saved = false
            try {
                vault.pairAndSave(target, code)
                saved = true
                prefs.edit().putString("origin", target).apply()
                status = "SAVED_SESSION_UNCONFIRMED"
                val info = vault.fetchSession(target)
                session = info; topics = info.topicKeys
                status = "SESSION_READY"
                detail = "登录已安全保存。选择授权话题、读取历史后可明确发送一条消息。"
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) {
                status = if (saved) "SAVED_SESSION_UNCONFIRMED" else "PAIR_FAILED"
                detail = if (saved) "登录已保存，但会话查询失败。请点恢复会话，不要重发配对码。"
                    else "配对未完成或未安全保存。请检查入口和配对码；网络中断时配对码可能已被使用，不能自动重发。"
            } finally { busy = false }
        }
    }
    fun restore() {
        if (busy) return
        val target = origin.trim()
        busy = true; clearHistory(); topics = emptyList(); status = "RESTORING"
        viewModelScope.launch {
            try {
                val info = vault.fetchSession(target)
                prefs.edit().putString("origin", target).apply()
                session = info; topics = info.topicKeys; status = "SESSION_READY"
                detail = "已从安全存储恢复并向服务端核对会话。没有重复配对或提交任务。"
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) {
                status = "RESTORE_FAILED"
                detail = "会话尚未配对、已过期或暂时无法核对。原凭据保留；不会自动重新配对。"
            } finally { busy = false }
        }
    }
    fun removeLocal() {
        if (busy) return
        val target = origin.trim()
        busy = true; clearHistory(); topics = emptyList()
        viewModelScope.launch {
            try {
                vault.remove(target)
                status = "REMOVED_LOCAL"
                detail = "仅移除了此入口在本机的登录。不撤销远端会话、不取消任务、不删除待确认提交。"
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (_: Exception) { status = "REMOVE_FAILED"; detail = "本机凭据未能确认删除，请重试。" }
            finally { busy = false }
        }
    }
}

@Composable
internal fun ConnectionScreen(model: ConnectionViewModel = viewModel()) {
    DisposableEffect(model) { onDispose { model.stopChat(); model.cancelRecovery() } }
    var confirmingSend by remember(model.origin, model.selectedTopic, model.confirmationGeneration, model.draft) { mutableStateOf(false) }
    if (confirmingSend) AlertDialog(
        onDismissRequest = { confirmingSend = false },
        title = { Text("确认原始消息首次提交") },
        text = { Text("只提交一次并先保存在本机。断网或 HTTP 202 不代表完成；未知提交先核对原回执，不自动重发。") },
        confirmButton = { TextButton(onClick = { confirmingSend = false; model.sendMessage() },
            modifier = Modifier.testTag("confirmOriginalSend")) { Text("确认发送一次") } },
        dismissButton = { TextButton(onClick = { confirmingSend = false }) { Text("取消") } }
    )
    // Not rememberSaveable: pairing codes must never enter saved instance state.
    var code by remember { mutableStateOf("") }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("连接 VCP", style = MaterialTheme.typography.headlineMedium)
        Text("选择授权话题后聊天。未知提交先核对原回执，重开不会自动重发。")
        OutlinedTextField(model.origin, model::updateOrigin, enabled = !model.busy,
            label = { Text("HTTPS 入口，不含 /mobile 路径") }, singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth().testTag("connectionOrigin"))
        OutlinedTextField(code, { if (it.length <= 64) code = it }, enabled = !model.busy,
            label = { Text("一次性配对码") }, singleLine = true,
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            modifier = Modifier.fillMaxWidth().testTag("pairingCode"))
        Button(onClick = { val submitted = code; code = ""; model.pair(submitted) },
            enabled = !model.busy, modifier = Modifier.testTag("pairConnection")) { Text("配对并安全保存") }
        OutlinedButton(onClick = model::restore, enabled = !model.busy,
            modifier = Modifier.testTag("restoreConnection")) { Text("恢复会话（只查询）") }
        Text(model.status, Modifier.testTag("connectionStatus"))
        Text(model.detail)
        if (model.busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        if (model.topics.isNotEmpty()) {
            Text("授权话题", style = MaterialTheme.typography.titleMedium)
            model.topics.forEach { topic ->
                TextButton(onClick = { model.selectTopic(topic) }, enabled = !model.busy || model.recoveryActive || model.chatActive,
                    modifier = Modifier.testTag("historyTopic_$topic")) {
                    Text(if (model.selectedTopic == topic) "✓ $topic" else topic)
                }
            }
        }
        if (model.selectedTopic != null) {
            OutlinedButton(onClick = model::recoverPending, enabled = !model.busy,
                modifier = Modifier.testTag("recoverPending")) { Text("核对待确认提交（只查询）") }
            Text(model.recoveryStatus, Modifier.testTag("recoveryStatus"))
            if (model.recoveryActive) {
                OutlinedButton(onClick = model::cancelRecovery,
                    modifier = Modifier.testTag("cancelRecovery")) { Text("取消核对") }
            }
            Text("消息", style = MaterialTheme.typography.titleMedium)
            OutlinedTextField(model.draft, model::updateDraft, enabled = !model.busy,
                label = { Text("消息内容") }, modifier = Modifier.fillMaxWidth().testTag("messageDraft"))
            Button(onClick = { confirmingSend = true }, enabled = model.canSend,
                modifier = Modifier.testTag("sendMessage")) { Text("确认后发送并查看回复") }
            Text(model.sendStatus, Modifier.testTag("sendStatus"))
            if (model.outgoingText.isNotEmpty()) Text("本次原消息：${model.outgoingText}")
            OutlinedButton(onClick = model::receiveReplies, enabled = !model.busy,
                modifier = Modifier.testTag("receiveReplies")) { Text("继续接收回复（只查询）") }
            if (model.chatActive) OutlinedButton(onClick = model::stopChat,
                modifier = Modifier.testTag("stopReceiving")) { Text("停止接收") }
            Text(model.replyStatus, Modifier.testTag("replyStatus"))
            Text("停止接收不会取消远端任务。发送回包丢失时先核对原回执，不自动重发；受理或运行中不等于AA完成。",
                style = MaterialTheme.typography.bodySmall)
            Text("话题历史（只读）", style = MaterialTheme.typography.titleMedium)
            Text(model.historyStatus, Modifier.testTag("historyStatus"))
            OutlinedButton(onClick = { model.readHistory(restart = true) }, enabled = !model.busy,
                modifier = Modifier.testTag("readHistory")) { Text("从头读取历史") }
            model.historyRows.forEach { row ->
                Text("${row.sequence} · ${historyPreview(row)}")
            }
            if (model.historyHasMore) {
                OutlinedButton(onClick = { model.readHistory() }, enabled = !model.busy,
                    modifier = Modifier.testTag("nextHistoryPage")) { Text("读取下一页") }
            }
            Text("仅内存保留最近400条。任务回执独立核对，不因读取历史而宣布任务完成。",
                style = MaterialTheme.typography.bodySmall)
            HorizontalDivider()
            Text("本机待确认请求（只读）", style = MaterialTheme.typography.titleSmall)
            OutlinedButton(onClick = model::inspectPending, enabled = !model.busy,
                modifier = Modifier.testTag("inspectPending")) { Text("核对本机待确认清单") }
            Text(model.pendingStatus, Modifier.testTag("pendingStatus"))
            model.pendingRows.forEach { row ->
                Text("${row.kind} · ${row.state} · ${row.recoveryMode} · ${row.requestId}")
            }
            Text("仅显示原请求标识，不显示内容；清单不是服务器收件或AA完成的证明。跨会话只允许查询。",
                style = MaterialTheme.typography.bodySmall)
        }
        OutlinedButton(onClick = model::removeLocal, enabled = !model.busy,
            modifier = Modifier.testTag("removeConnection")) { Text("移除本机登录") }
        Text("只填写你信任的服务入口。配对码仅用于一次请求；不放入日志、聊天历史或普通设置。",
            style = MaterialTheme.typography.bodySmall)
    }
}

@Composable
fun NativeRoot() {
    var connection by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize().systemBarsPadding()) {
        Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(onClick = { connection = false }) { Text("工程验证") }
            TextButton(onClick = { connection = true }, modifier = Modifier.testTag("connectionTab")) { Text("连接 VCP") }
        }
        Box(Modifier.weight(1f)) { if (connection) ConnectionScreen() else ProbeScreen() }
    }
}

/** Native Text never interprets HTML/JSON as markup or executable instructions. */
private fun historyPreview(row: NativeHistoryRow): String {
    if (row.deleted) return "[消息已删除]"
    return try {
        val objectValue = JSONObject(row.messageJson ?: return "[消息无法预览]")
        val role = objectValue.optString("role", "消息").take(24)
        val content = objectValue.opt("content")
        "$role: ${if (content is String) content.take(400) else "[非纯文本内容]"}"
    } catch (_: Exception) { "[消息无法预览]" }
}
