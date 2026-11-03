import XCTest
@testable import VCPNative

final class NativeTopicSendGuardTests: XCTestCase {
    func testOnlyStrictCompleteAuthorizedHistoryEnablesOriginalSend() {
        let good = #"{"historyReady":true,"executable":true}"#
        XCTAssertTrue(NativeTopicSendGuard.allowsSend(topicStateJson: good, hasMore: false))
        XCTAssertFalse(NativeTopicSendGuard.allowsSend(topicStateJson: good, hasMore: true))
        for bad in [
            #"{"historyReady":false,"executable":true}"#,
            #"{"historyReady":true,"executable":false}"#,
            #"{"historyReady":1,"executable":true}"#,
            #"{"historyReady":true,"executable":1}"#,
            #"{"historyReady":"true","executable":true}"#,
            #"{"historyReady":true,"executable":"true"}"#,
            #"{"historyReady":null,"executable":true}"#,
            #"{"historyReady":true}"#, #"{"executable":true}"#,
            #"{"topic":{"historyReady":true,"executable":true}}"#,
            #"{}"#, #"[]"#, #"null"#, "", "{broken"
        ] {
            XCTAssertFalse(NativeTopicSendGuard.allowsSend(topicStateJson: bad, hasMore: false), bad)
        }
    }
}
