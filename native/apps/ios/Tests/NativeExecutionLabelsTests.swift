import XCTest
@testable import VCPNative

final class NativeExecutionLabelsTests: XCTestCase {
    func testStateTokensNeverExposeTaskPayloadsOrIDs() {
        let raw = #"[{"id":"private-job","state":"running","body":"private payload","extension":{"secret":"private"}},{"state":"review_required"}]"#
        let visible = NativeExecutionLabels.project(raw)
        XCTAssertEqual(visible, ["服务端状态：running", "服务端状态：review_required"])
        XCTAssertFalse(visible.joined().contains("private"))
    }
    func testMalformedAndUnsafeStatesAreNotCompletionClaims() {
        XCTAssertEqual(NativeExecutionLabels.project("not-json"), ["任务状态快照无效，请在服务端核对"])
        XCTAssertEqual(NativeExecutionLabels.project(#"[{"state":"ok\nsecret"},{"state":123},{"state":"🔥"}]"#),
            ["服务端状态：未识别", "服务端状态：未识别", "服务端状态：未识别"])
    }
    func testBoundedProjectionAndEmptySnapshot() {
        XCTAssertEqual(NativeExecutionLabels.project("[]"), [])
        let records = (0..<35).map { _ in ["state": "running"] }
        let data = try! JSONSerialization.data(withJSONObject: records)
        let visible = NativeExecutionLabels.project(String(decoding: data, as: UTF8.self))
        XCTAssertEqual(visible.count, 25)
        XCTAssertEqual(visible.last, "另有 11 项状态未显示")
    }
    func testOversizedProjectionDoesNotReadPrivateExtension() {
        let large = #"[{"state":"running","body":""# + String(repeating: "x", count: 131_072) + #""}]"#
        XCTAssertEqual(NativeExecutionLabels.project(large), ["任务状态快照过大，请在服务端核对"])
    }
}
