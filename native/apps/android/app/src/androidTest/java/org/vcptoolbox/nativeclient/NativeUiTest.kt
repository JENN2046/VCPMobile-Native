package org.vcptoolbox.nativeclient

import androidx.compose.ui.test.*
import android.util.Log
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NativeUiTest {
    @get:Rule val ui = createAndroidComposeRule<MainActivity>()
    private fun awaitStatus(expected: String) {
        try {
            ui.waitUntil(15_000) {
                ui.onAllNodes(hasTestTag("status") and hasText(expected)).fetchSemanticsNodes().isNotEmpty()
            }
        } catch (failure: Throwable) {
            throw AssertionError("Expected $expected. Synthetic UI state:\n" + ui.onRoot(useUnmergedTree = true).printToString(), failure)
        }
    }
    @Test fun successAndCallbackReachNativeUi() {
        ui.onNodeWithTag("success").performScrollTo().performClick()
        awaitStatus("SUCCESS")
        ui.onNodeWithTag("progress").assertTextEquals("进度：3")
    }
    @Test fun structuredErrorReachesNativeUi() {
        ui.onNodeWithTag("invalid").performScrollTo().performClick()
        awaitStatus("ERROR")
        ui.onNodeWithText("steps", substring = true).assertExists()
    }
    @Test fun cancellationReachesNativeUi() {
        // Per-step markers survive instrumentation timeouts and identify
        // the exact action that blocked without weakening any assertion.
        Log.i("VCPNativeUITest", "cancel: before slow click")
        ui.onNodeWithTag("slow").performScrollTo().performClick()
        Log.i("VCPNativeUITest", "cancel: slow click returned")
        awaitStatus("RUNNING")
        Log.i("VCPNativeUITest", "cancel: RUNNING observed")
        ui.onNodeWithTag("cancel").performScrollTo().performClick()
        Log.i("VCPNativeUITest", "cancel: cancel click returned")
        awaitStatus("CANCELLED")
        Log.i("VCPNativeUITest", "cancel: CANCELLED observed")
        ui.onNodeWithTag("success").assertIsEnabled()
    }
    @Test fun recreatedActivityCanCallCore() {
        ui.activityRule.scenario.recreate()
        ui.onNodeWithTag("success").performScrollTo().performClick()
        awaitStatus("SUCCESS")
    }
}
