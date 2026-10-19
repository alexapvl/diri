import XCTest

/// Opt-in: captures the session screen's states against a fixture or real
/// gateway, for PR media. Skipped unless `DIRI_SHOTS_URL` (a pairing link) and
/// `DIRI_SHOTS_DIR` (where PNGs are written) reach the runner:
///
///     TEST_RUNNER_DIRI_SHOTS_URL='http://127.0.0.1:7391/?token=…' \
///     TEST_RUNNER_DIRI_SHOTS_DIR=/tmp/shots \
///     TEST_RUNNER_DIRI_SHOTS_SESSION=s_tui \
///     xcodebuild … -only-testing:DiriPhoneUITests/ScreenshotTests test
///
/// It answers the speech and microphone prompts on the way, so later headless
/// launches open voice mode without them. Nothing is left running.
@MainActor
final class ScreenshotTests: XCTestCase {
    private var springboard: XCUIApplication { XCUIApplication(bundleIdentifier: "com.apple.springboard") }

    func testCaptureSessionStates() throws {
        let env = ProcessInfo.processInfo.environment
        guard let link = env["DIRI_SHOTS_URL"], let dir = env["DIRI_SHOTS_DIR"] else {
            throw XCTSkip("Set TEST_RUNNER_DIRI_SHOTS_URL and TEST_RUNNER_DIRI_SHOTS_DIR.")
        }
        let session = env["DIRI_SHOTS_SESSION"] ?? "s_tui"
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)

        let app = XCUIApplication()
        app.launchArguments = ["-endpoint", link, "-session", session]
        app.launch()
        let terminal = app.descendants(matching: .any)["terminal-text"]
        XCTAssertTrue(terminal.waitForExistence(timeout: 20))
        settle(2)
        save(dir, "session")

        app.buttons["show-sessions"].tap()
        settle(1)
        save(dir, "drawer")
        app.descendants(matching: .any)["session-row-\(session)"].firstMatch.tap()
        settle(1)

        app.buttons["terminal-keyboard"].tap()
        settle(1.5)
        // A fresh simulator introduces slide-to-type first.
        if app.buttons["Continue"].exists { app.buttons["Continue"].tap(); settle(1) }
        save(dir, "keyboard")
        app.buttons["key-hide"].tap()
        settle(1)

        let field = app.textViews["composer-field"].exists ? app.textViews["composer-field"] : app.textFields["composer-field"]
        field.tap()
        if app.buttons["Continue"].waitForExistence(timeout: 1.5) { app.buttons["Continue"].tap() }
        field.typeText("Fling through the scrollback")
        settle(1)
        save(dir, "composer")
        app.buttons["composer-send"].tap()
        settle(1)
        app.descendants(matching: .any)["terminal-text"].tap()
        app.buttons["key-hide"].tap()
        settle(1)

        app.buttons["voice-mode-open"].tap()
        answerAlerts(for: 6)
        settle(2)
        save(dir, "voice")
        // No tap after this: the working indicator animates, and XCUITest would
        // wait for an idle app that never comes.
        app.terminate()
    }

    private func save(_ dir: String, _ name: String) {
        let shot = XCUIScreen.main.screenshot()
        try? shot.pngRepresentation.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func settle(_ seconds: TimeInterval) {
        RunLoop.current.run(until: Date().addingTimeInterval(seconds))
    }

    private func answerAlerts(for seconds: TimeInterval) {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            for title in ["Allow", "OK", "Allow While Using App"] {
                let button = springboard.alerts.buttons[title]
                if button.exists { button.tap() }
            }
            settle(0.3)
        }
    }
}
