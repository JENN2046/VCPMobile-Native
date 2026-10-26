package org.vcptoolbox.nativeclient

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ConnectionUiTest {
    @get:Rule val ui = createAndroidComposeRule<MainActivity>()
    private fun open() {
        ui.onNodeWithTag("connectionTab").performClick()
        ui.onNodeWithTag("connectionOrigin").performTextReplacement("https://ui-pairing.example.test")
    }
    private fun awaitStatus(value: String) {
        ui.waitUntil(15_000) {
            ui.onAllNodes(hasTestTag("connectionStatus") and hasText(value)).fetchSemanticsNodes().isNotEmpty()
        }
    }
    @Test fun invalidPairingIsLocalAndClearsCode() {
        open()
        ui.onNodeWithTag("pairingCode").performTextInput("invalid")
        ui.onNodeWithTag("pairConnection").performScrollTo().performClick()
        awaitStatus("PAIR_FAILED")
        ui.onNodeWithTag("pairingCode").assert(hasText(""))
        ui.onNodeWithTag("restoreConnection").assertIsEnabled()
    }
    @Test fun unpairedRestoreDoesNotStartPairing() {
        open()
        ui.onNodeWithTag("restoreConnection").performScrollTo().performClick()
        awaitStatus("RESTORE_FAILED")
        ui.onNodeWithTag("pairConnection").assertIsEnabled()
        ui.onNodeWithTag("readHistory").assertDoesNotExist()
        ui.onNodeWithTag("recoverPending").assertDoesNotExist()
    }
}
