import XCTest

/// Opt-in end-to-end run against a real `diri-web`. Skipped unless the test
/// runner sees `DIRI_E2E_URL`. xcodebuild forwards `TEST_RUNNER_`-prefixed
/// variables to the runner with the prefix stripped:
///
///     TEST_RUNNER_DIRI_E2E_URL='http://mac:7380/?token=…' \
///     TEST_RUNNER_DIRI_E2E_CWD=/path/to/scratch/repo \
///     TEST_RUNNER_DIRI_E2E_AUDIO=/path/to/reply.aiff \
///     xcodebuild … -only-testing:DiriPhoneUITests test
///
/// - `DIRI_E2E_URL` (required): pairing link including the token.
/// - `DIRI_E2E_CWD` (required): folder on "this Mac" for a new Claude session,
///   started in place (no worktree).
/// - `DIRI_E2E_PROMPT` (optional): what to send from the composer.
/// - `DIRI_E2E_AUDIO` (optional): audio file the app dictates instead of the
///   microphone (`say -o reply.aiff "thanks"`), proving the listen → send leg.
///
/// Cleanup: the session it starts is left running for inspection; kill it from
/// Diri. The pairing is saved in the simulator's keychain.
@MainActor
final class EndToEndTests: XCTestCase {
    private var app: XCUIApplication!
    private var springboard: XCUIApplication { XCUIApplication(bundleIdentifier: "com.apple.springboard") }

    override nonisolated func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testPairStartSendAndHearTheTurn() throws {
        let env = ProcessInfo.processInfo.environment
        guard let link = env["DIRI_E2E_URL"], !link.isEmpty else {
            throw XCTSkip("Set TEST_RUNNER_DIRI_E2E_URL to run the end-to-end test.")
        }
        guard let cwd = env["DIRI_E2E_CWD"], !cwd.isEmpty else {
            throw XCTSkip("Set TEST_RUNNER_DIRI_E2E_CWD to a folder on the Mac.")
        }
        let prompt = env["DIRI_E2E_PROMPT"] ?? "Reply with one short sentence: voice check ok."
        let audio = env["DIRI_E2E_AUDIO"].flatMap { $0.isEmpty ? nil : $0 }

        app = XCUIApplication()
        app.launchEnvironment["DIRI_E2E_CWD"] = cwd
        if let audio { app.launchEnvironment["DIRI_VOICE_TEST_AUDIO"] = audio }
        app.launch()

        // Pair through the paste-link path, whether or not the simulator is
        // already paired with something.
        if app.buttons["connection-menu"].waitForExistence(timeout: 5) {
            app.buttons["connection-menu"].tap()
            app.buttons["add-gateway"].tap()
        } else if app.buttons["Already set up? Scan a code"].waitForExistence(timeout: 5) {
            app.buttons["Already set up? Scan a code"].tap()
        }
        let paste = app.descendants(matching: .any)["pairing-paste"]
        XCTAssertTrue(paste.waitForExistence(timeout: 10))
        paste.tap()
        let field = app.textFields["pairing-link-field"].exists
            ? app.textFields["pairing-link-field"] : app.textViews["pairing-link-field"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        field.typeText(link)
        app.buttons["pairing-connect"].tap()

        // New Claude session on this Mac, in place.
        let newSession = app.buttons["new-session"]
        XCTAssertTrue(newSession.waitForExistence(timeout: 30), "pairing did not reach the session list")
        newSession.tap()
        let worktree = app.switches["new-session-worktree"]
        XCTAssertTrue(worktree.waitForExistence(timeout: 10))
        if (worktree.value as? String) == "1" { worktree.switches.firstMatch.tap() }
        let start = app.buttons["new-session-start"]
        XCTAssertTrue(waitUntil(timeout: 30) { start.isEnabled }, "Start never enabled — is Claude installed?")
        start.tap()

        // Send from the composer.
        XCTAssertTrue(waitUntil(timeout: 120) {
            app.staticTexts["terminal-text"].exists && app.buttons["voice-mode-open"].isEnabled
        }, "the new session never opened")
        let input = app.textViews["composer-field"].exists ? app.textViews["composer-field"] : app.textFields["composer-field"]
        // Give the agent a moment to draw its prompt before typing into it.
        sleep(5)
        input.tap()
        input.typeText(prompt)
        app.buttons["composer-send"].tap()

        // Voice mode hears the turn finish and reads it aloud.
        app.buttons["voice-mode-open"].tap()
        let agentText = app.staticTexts["voice-agent-text"]
        XCTAssertTrue(agentText.waitForExistence(timeout: 10))
        let spoken = app.descendants(matching: .any)["voice-last-spoken"]
        XCTAssertTrue(waitUntil(timeout: 240) { !spoken.label.isEmpty }, "no turn was read aloud")
        XCTAssertFalse(agentText.label.isEmpty)
        XCTAssertNotEqual(agentText.label, "Waiting for the agent…")
        XCTAssertEqual(spoken.label, agentText.label)

        if audio != nil {
            // The test audio is dictated after speech ends and sent on silence.
            let sent = app.staticTexts["voice-last-sent"]
            XCTAssertTrue(waitUntil(timeout: 120) { sent.exists }, "dictated reply was never sent")
        }
    }

    /// Polls `condition`, answering permission alerts (speech, microphone,
    /// local network) on the way.
    private func waitUntil(timeout: TimeInterval, _ condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            for title in ["Allow", "OK", "Allow While Using App"] {
                let button = springboard.alerts.buttons[title]
                if button.exists { button.tap() }
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        return condition()
    }
}
