package org.vcptoolbox.nativeclient

/** Ephemeral confirmation captured at the user click, never persisted.
 * No sender is authorized if the draft or protected context has changed. */
internal data class NativeSendConfirmation(
    val origin: String,
    val entryId: String,
    val sessionMarker: String,
    val topic: String,
    val viewGeneration: Long,
    val text: String,
) {
    fun matches(currentOrigin: String, currentEntryId: String?, currentSessionMarker: String?,
                currentTopic: String?, currentViewGeneration: Long, currentText: String): Boolean =
        origin == currentOrigin && entryId == currentEntryId &&
        sessionMarker == currentSessionMarker && topic == currentTopic &&
        viewGeneration == currentViewGeneration && text == currentText
}
