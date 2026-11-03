import XCTest
@testable import VCPNative

final class NativeSendConfirmationTests: XCTestCase {
    func testOriginalConfirmationCannotCrossContextOrDraft() {
        let captured = NativeSendConfirmation(origin: "https://one.example.test", entryId: "entry-1",
            sessionMarker: "session-1", topic: "topic-1", viewGeneration: 17,
            text: "first draft 🧣")
        func same(_ origin: String = "https://one.example.test", _ entry: String? = "entry-1",
                  _ session: String? = "session-1", _ topic: String? = "topic-1",
                  _ generation: UInt64 = 17, _ text: String = "first draft 🧣") -> Bool {
            captured.matches(origin: origin, entryId: entry, sessionMarker: session,
                topic: topic, viewGeneration: generation, text: text)
        }
        XCTAssertTrue(same())
        XCTAssertFalse(same("https://two.example.test"))
        XCTAssertFalse(same("https://one.example.test", "entry-2"))
        XCTAssertFalse(same("https://one.example.test", "entry-1", "session-2"))
        XCTAssertFalse(same("https://one.example.test", "entry-1", "session-1", "topic-2"))
        XCTAssertFalse(same("https://one.example.test", "entry-1", "session-1", "topic-1", 18))
        XCTAssertFalse(same("https://one.example.test", "entry-1", "session-1", "topic-1", 17, "edited"))
        XCTAssertFalse(same("https://one.example.test", nil))
    }
}
