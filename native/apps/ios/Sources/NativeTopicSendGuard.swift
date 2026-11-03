import Foundation
import CoreFoundation

/// A source-backed, fail-closed submission-readiness gate. It consumes only
/// the server's authenticated Topic-state JSON, not a local UI/AA projection.
/// Failed/unread/incomplete history must never implicitly authorize a send.
enum NativeTopicSendGuard {
    static func allowsSend(topicStateJson: String, hasMore: Bool) -> Bool {
        guard !hasMore, let data = topicStateJson.data(using: .utf8),
              data.count <= 16_384,
              let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let ready = strictBoolean(object["historyReady"]),
              let executable = strictBoolean(object["executable"]) else { return false }
        return ready && executable
    }

    private static func strictBoolean(_ value: Any?) -> Bool? {
        guard let number = value as? NSNumber,
              CFGetTypeID(number) == CFBooleanGetTypeID() else { return nil }
        return number.boolValue
    }
}
