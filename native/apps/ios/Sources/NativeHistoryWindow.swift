import Foundation

/// Reconciles recent authorized history into an in-memory view only.
/// A central revision or deletion may replace an ID only at a greater sequence.
/// Never mutates a pending submission or infers AA completion.
enum NativeHistoryWindowError: Error {
    case staleRevision
}

enum NativeHistoryWindow {
    static func merge(_ previous: [NativeHistoryRow], incoming: [NativeHistoryRow]) throws -> [NativeHistoryRow] {
        var rows: [String: NativeHistoryRow] = [:]
        for row in previous {
            if let prior = rows[row.messageId], row.sequence <= prior.sequence {
                throw NativeHistoryWindowError.staleRevision
            }
            rows[row.messageId] = row
        }
        var incomingIds = Set<String>()
        for row in incoming {
            guard incomingIds.insert(row.messageId).inserted else {
                throw NativeHistoryWindowError.staleRevision
            }
            if let prior = rows[row.messageId], row.sequence <= prior.sequence {
                throw NativeHistoryWindowError.staleRevision
            }
            rows[row.messageId] = row
        }
        return Array(rows.values.sorted {
            if $0.sequence != $1.sequence { return $0.sequence < $1.sequence }
            return $0.messageId < $1.messageId
        }.suffix(400))
    }
}
