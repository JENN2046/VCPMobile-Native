package org.vcptoolbox.nativeclient

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class NativeUiTest {
    @get:Rule val ui = createAndroidComposeRule<MainActivity>()
    private fun awaitStatus(expected: String) {
        ui.waitUntil(15_000) {
            ui.onAllNodes(hasTestTag("status") and hasText(expected)).fetchSemanticsNodes().isNotEmpty()
        }
    }
    @Test fun successAndCallbackReachNativeUi() {
        ui.onNodeWithTag("success").performClick()
        awaitStatus("SUCCESS")
        ui.onNodeWithTag("progress").assertTextEquals("进度：3")
    }
    @Test fun structuredErrorReachesNativeUi() {
        ui.onNodeWithTag("invalid").performClick()
        awaitStatus("ERROR")
        ui.onNodeWithText("steps", substring = true).assertExists()
    }
    @Test fun cancellationReachesNativeUi() {
        ui.onNodeWithTag("slow").performClick()
        awaitStatus("RUNNING")
        ui.onNodeWithTag("cancel").performClick()
        awaitStatus("CANCELLED")
        ui.onNodeWithTag("success").assertIsEnabled()
    }
    @Test fun recreatedActivityCanCallCore() {
        ui.activityRule.scenario.recreate()
        ui.onNodeWithTag("success").performClick()
        awaitStatus("SUCCESS")
    }
}
