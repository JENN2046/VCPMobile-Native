import XCTest

final class NativeUiTests: XCTestCase {
    private func app() -> XCUIApplication {
        let app = XCUIApplication()
        app.launch()
        return app
    }
    private func expectStatus(_ value: String, in app: XCUIApplication) {
        let element = app.staticTexts["status"]
        let predicate = NSPredicate(format: "label == %@", value)
        expectation(for: predicate, evaluatedWith: element)
        waitForExpectations(timeout: 15)
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = value
        shot.lifetime = .keepAlways
        add(shot)
    }
    func testSuccessAndCallbacks() {
        let app = app()
        app.buttons["success"].tap()
        expectStatus("SUCCESS", in: app)
        XCTAssertEqual(app.staticTexts["progress"].label, "进度：3")
    }
    func testStructuredError() {
        let app = app()
        app.buttons["invalid"].tap()
        expectStatus("ERROR", in: app)
    }
    func testCancelAndRestart() {
        let app = app()
        app.buttons["slow"].tap()
        expectStatus("RUNNING", in: app)
        app.buttons["cancel"].tap()
        expectStatus("CANCELLED", in: app)
        app.buttons["success"].tap()
        expectStatus("SUCCESS", in: app)
    }
}
