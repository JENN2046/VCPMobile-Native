import XCTest

final class ConnectionUiTests: XCTestCase {
    private func open() -> XCUIApplication {
        let app = XCUIApplication(); app.launch()
        app.tabBars.buttons["连接 VCP"].tap()
        let origin = app.textFields["connectionOrigin"]
        origin.tap()
        // Select all handles a retained non-secret preference from another run.
        if let existing = origin.value as? String, existing.hasPrefix("https://") {
            origin.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: existing.count))
        }
        origin.typeText("https://ui-pairing.example.test\n")
        return app
    }
    private func expect(_ value: String, in app: XCUIApplication) {
        expectation(for: NSPredicate(format: "label == %@", value), evaluatedWith: app.staticTexts["connectionStatus"])
        waitForExpectations(timeout: 15)
    }
    func testInvalidPairingIsLocalAndClearsCode() {
        let app = open()
        let code = app.secureTextFields["pairingCode"]; code.tap(); code.typeText("invalid\n")
        app.buttons["pairConnection"].tap()
        expect("PAIR_FAILED", in: app)
        XCTAssertTrue(app.buttons["restoreConnection"].isEnabled)
        XCTAssertFalse((code.value as? String ?? "").contains("invalid"))
    }
    func testUnpairedRestoreDoesNotStartPairing() {
        let app = open()
        app.buttons["restoreConnection"].tap()
        expect("RESTORE_FAILED", in: app)
        XCTAssertTrue(app.buttons["pairConnection"].isEnabled)
        XCTAssertFalse(app.buttons["readHistory"].exists)
        XCTAssertFalse(app.buttons["inspectPending"].exists)
        XCTAssertFalse(app.buttons["recoverPending"].exists)
    }
}
