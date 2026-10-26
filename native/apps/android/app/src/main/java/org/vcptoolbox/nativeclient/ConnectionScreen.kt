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

/** Only non-secret origin preference is persisted outside the protected vault. */
internal class ConnectionViewModel(app: Application) : AndroidViewModel(app) {
    private val prefs = app.getSharedPreferences("portal-origin-v1", Context.MODE_PRIVATE)
    private val vault = PortalSessionVault(app)
    var origin by mutableStateOf(prefs.getString("origin", "") ?: ""); private set
    var status by mutableStateOf("NOT_CONNECTED"); private set
    var detail by mutableStateOf("输入受信任 VCP 的 HTTPS 入口和新配对码。启动不会自动联网。"); private set
    var topics by mutableStateOf<List<String>>(emptyList()); private set
    var busy by mutableStateOf(false); private set

    fun updateOrigin(value: String) {
        if (busy) return
        origin = value; topics = emptyList(); status = "NOT_CONNECTED"
        detail = "入口已改变，请配对或核对该入口的已保存会话。"
    }
    fun pair(code: String) {
        if (busy) return
        val target = origin.trim()
        busy = true; topics = emptyList(); status = "PAIRING"
        viewModelScope.launch {
            var saved = false
            try {
                vault.pairAndSave(target, code)
                saved = true
                prefs.edit().putString("origin", target).apply()
                status = "SAVED_SESSION_UNCONFIRMED"
                val info = vault.fetchSession(target)
                topics = info.topicKeys
                status = "SESSION_READY"
                detail = "登录已安全保存，授权话题已读取。当前不发送聊天或任务。"
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
        busy = true; topics = emptyList(); status = "RESTORING"
        viewModelScope.launch {
            try {
                val info = vault.fetchSession(target)
                prefs.edit().putString("origin", target).apply()
                topics = info.topicKeys; status = "SESSION_READY"
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
        busy = true; topics = emptyList()
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
    // Not rememberSaveable: pairing codes must never enter saved instance state.
    var code by remember { mutableStateOf("") }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("连接 VCP", style = MaterialTheme.typography.headlineMedium)
        Text("M2 · 配对与会话恢复；尚不是完整聊天客户端。")
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
            model.topics.forEach { Text(it) }
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
