import XCTest
@testable import VCPNative

final class NativeHistoryWindowTests: XCTestCase {
    private func row(_ sequence: UInt64, _ id: String = "reply", deleted: Bool = false) -> NativeHistoryRow {
        NativeHistoryRow(sequence: sequence, messageId: id,
            deleted: deleted, messageJson: deleted ? nil : "{\"content\":\"revision \(sequence)\"}")
    }
    func testRevisionAndTombstoneReplaceSameId() throws {
        let first = try NativeHistoryWindow.merge([], incoming: [row(1)])
        let updated = try NativeHistoryWindow.merge(first, incoming: [row(2)])
        XCTAssertEqual(updated.count, 1)
        XCTAssertEqual(updated[0].sequence, 2)
        let deleted = try NativeHistoryWindow.merge(updated, incoming: [row(3, deleted: true)])
        XCTAssertTrue(deleted[0].deleted)
        XCTAssertNil(deleted[0].messageJson)
    }
    func testStaleAndDuplicateRevisionsCannotOverwrite() throws {
        let current = [row(9)]
        XCTAssertThrowsError(try NativeHistoryWindow.merge(current, incoming: [row(8)]))
        XCTAssertThrowsError(try NativeHistoryWindow.merge(current, incoming: [row(9)]))
        XCTAssertThrowsError(try NativeHistoryWindow.merge([], incoming: [row(1), row(2)]))
    }
    func testBoundedHistoryRetainsLatestRows() throws {
        let rows = try NativeHistoryWindow.merge([], incoming: (1...500).map { row(UInt64($0), "m_\($0)") })
        XCTAssertEqual(rows.count, 400)
        XCTAssertEqual(rows.first?.sequence, 101)
        XCTAssertEqual(rows.last?.sequence, 500)
    }
    func testRevisionOnOlderIdentityGetsLatestSequenceOrder() throws {
        let original = [row(5, "a"), row(6, "b")]
        let merged = try NativeHistoryWindow.merge(original, incoming: [row(7, "a", deleted: true)])
        XCTAssertEqual(merged.map(\.messageId), ["b", "a"])
        XCTAssertTrue(merged.last!.deleted)
    }
}
