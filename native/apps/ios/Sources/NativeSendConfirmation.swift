import Foundation

/// One ephemeral user confirmation for one original draft and Topic context.
/// This value is not an execution ticket and is never persisted.
struct NativeSendConfirmation: Equatable {
    let origin: String
    let entryId: String
    let sessionMarker: String
    let topic: String
    let viewGeneration: UInt64
    let text: String

    func matches(origin currentOrigin: String, entryId currentEntryId: String?,
                 sessionMarker currentSessionMarker: String?, topic currentTopic: String?,
                 viewGeneration currentGeneration: UInt64, text currentText: String) -> Bool {
        origin == currentOrigin && entryId == currentEntryId &&
        sessionMarker == currentSessionMarker && topic == currentTopic &&
        viewGeneration == currentGeneration && text == currentText
    }
}
