package org.vcptoolbox.nativeclient

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.vcptoolbox.nativecore.*

class ProbeViewModel : ViewModel() {
    var status by mutableStateOf("IDLE"); private set
    var detail by mutableStateOf("只验证原生界面与 Rust 调用，尚未连接 NativeSync。"); private set
    var progress by mutableStateOf(0u); private set
    var running by mutableStateOf(false); private set
    private var current: BridgeProbe? = null
    private var generation = 0L

    fun start(mode: String) {
        if (running) return
        val ticket = ++generation
        val probe = BridgeProbe()
        current = probe
        running = true
        status = "RUNNING"
        progress = 0u
        val request = when (mode) {
            "invalid" -> ProbeRequest("invalid", 0u, 0u)
            "slow" -> ProbeRequest("cancel", 128u, 1000u)
            else -> ProbeRequest("跨端验证", 3u, 20u)
        }
        val listener = object : ProbeListener {
            override fun onProgress(value: ProbeProgress) {
                viewModelScope.launch(Dispatchers.Main.immediate) {
                    if (generation == ticket) progress = value.completed
                }
            }
        }
        viewModelScope.launch {
            try {
                val result = probe.run(request, listener)
                if (generation == ticket) {
                    progress = result.completed
                    status = "SUCCESS"
                    detail = "共享 Rust 核心返回：${result.label} · ${result.completed}"
                }
            } catch (e: CancellationException) {
                probe.cancel()
                throw e
            } catch (e: Exception) {
                if (generation == ticket) {
                    val message = e.toString()
                    status = if (message.contains("cancel", ignoreCase = true)) "CANCELLED" else "ERROR"
                    detail = message
                }
            } finally {
                if (generation == ticket) { current = null; running = false }
                probe.destroy()
            }
        }
    }
    fun cancel() { current?.cancel() }
    override fun onCleared() { current?.cancel(); super.onCleared() }
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { MaterialTheme { Surface(Modifier.fillMaxSize()) { ProbeScreen() } } }
    }
}

@Composable
fun ProbeScreen(model: ProbeViewModel = viewModel()) {
    Column(Modifier.fillMaxSize().systemBarsPadding().verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text("VCP 原生开发版", style = MaterialTheme.typography.headlineMedium)
        Text("M1 · 共享核心验证", style = MaterialTheme.typography.titleMedium)
        Text("这是工程验证界面，不是已接通服务的聊天客户端。")
        Text(coreInfo(), style = MaterialTheme.typography.bodySmall)
        Text(model.status, Modifier.testTag("status"), style = MaterialTheme.typography.titleLarge)
        Text("进度：${model.progress}", Modifier.testTag("progress"))
        Text(model.detail, Modifier.testTag("detail"))
        Button(onClick = { model.start("success") }, enabled = !model.running, modifier = Modifier.testTag("success")) { Text("运行成功路径") }
        Button(onClick = { model.start("invalid") }, enabled = !model.running, modifier = Modifier.testTag("invalid")) { Text("验证错误返回") }
        Button(onClick = { model.start("slow") }, enabled = !model.running, modifier = Modifier.testTag("slow")) { Text("运行可取消路径") }
        OutlinedButton(onClick = model::cancel, enabled = model.running, modifier = Modifier.testTag("cancel")) { Text("取消本地验证") }
    }
}
