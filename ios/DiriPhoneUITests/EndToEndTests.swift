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
///   Needs a real device: no speech recogniser starts in the simulator.
/// - `DIRI_E2E_TRANSCRIPT` (optional): text the app "hears" instead, for the
///   simulator; proves everything after recognition (silence → send → reply).
/// - `DIRI_E2E_SCROLL_SESSION` (optional): id of a session on the same gateway
///   with over a thousand lines of scrollback; the test switches to it through
///   the drawer, flings back through its history and switches back.
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
        let transcript = env["DIRI_E2E_TRANSCRIPT"].flatMap { $0.isEmpty ? nil : $0 }
        if let transcript { app.launchEnvironment["DIRI_VOICE_TEST_TRANSCRIPT"] = transcript }
        app.launch()

        // Pair through the paste-link path, whether or not the simulator is
        // already paired with something.
        if app.buttons["connection-menu"].waitForExistence(timeout: 5) {
            app.buttons["connection-menu"].tap()
            app.buttons["add-gateway"].tap()
        } else if app.buttons["Already set up? Scan a code"].waitForExistence(timeout: 5) {
            app.buttons["Already set up? Scan a code"].tap()
        }
        let paste = app.descendants(matching: .any).matching(identifier: "pairing-paste").firstMatch
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

        // The session opens on its terminal, drawn as a TUI: Claude's welcome
        // box and status line carry colour.
        let terminal = app.descendants(matching: .any)["terminal-text"]
        XCTAssertTrue(waitUntil(timeout: 120) {
            terminal.exists && app.buttons["voice-mode-open"].isEnabled
        }, "the new session never opened")
        XCTAssertTrue(waitUntil(timeout: 60) { Self.coloured(terminal) > 0 },
                      "no coloured cells: \(terminal.value as? String ?? "")")
        // Give the agent a moment to draw its prompt before typing into it.
        sleep(5)

        // Raw keys straight to the PTY: typed text echoes in Claude's input
        // box, and sticky ctrl + u clears it again.
        app.buttons["terminal-keyboard"].tap()
        Self.dismissKeyboardTips(app)
        XCTAssertTrue(app.buttons["key-ctrl"].waitForExistence(timeout: 5), "no key accessory row")
        app.typeText("raw keys")
        XCTAssertTrue(waitUntil(timeout: 15) { terminal.label.contains("raw keys") },
                      "typed keys never reached the PTY")
        app.buttons["key-ctrl"].tap()
        app.typeText("u")
        XCTAssertTrue(waitUntil(timeout: 15) { !terminal.label.contains("raw keys") }, "ctrl-u did not clear the line")
        app.buttons["key-hide"].tap()

        // Scrollback: switch to a session with long history through the
        // drawer, fling back through it, and come back.
        if let scrollID = env["DIRI_E2E_SCROLL_SESSION"], !scrollID.isEmpty {
            let claudeID = Self.sessionRow(app, excluding: scrollID)
            app.buttons["show-sessions"].tap()
            let row = app.descendants(matching: .any)["session-row-\(scrollID)"].firstMatch
            XCTAssertTrue(row.waitForExistence(timeout: 10), "the drawer does not list the scroll session")
            row.tap()
            XCTAssertTrue(waitUntil(timeout: 20) { Self.total(terminal) > 1000 }, "no history: \(terminal.value as? String ?? "")")
            let bottom = Self.top(terminal)
            for _ in 0 ..< 4 { terminal.swipeDown(velocity: .fast) }
            XCTAssertTrue(waitUntil(timeout: 10) { Self.top(terminal) < bottom - 200 },
                          "history did not scroll: \(terminal.value as? String ?? "")")
            app.buttons["show-sessions"].tap()
            if let claudeID {
                app.descendants(matching: .any)[claudeID].firstMatch.tap()
            }
            XCTAssertTrue(app.buttons["voice-mode-open"].waitForExistence(timeout: 10))
        }

        // Send from the composer, then open voice mode straight away: it
        // reads turns that finish while it is open.
        let input = app.textViews["composer-field"].exists ? app.textViews["composer-field"] : app.textFields["composer-field"]
        XCTAssertTrue(input.waitForExistence(timeout: 5))
        input.tap()
        Self.dismissKeyboardTips(app)
        input.typeText(prompt)
        app.buttons["composer-send"].tap()
        XCTAssertTrue(app.buttons["voice-mode-open"].waitForExistence(timeout: 60), "the draft was not sent")

        // Voice mode hears the turn finish and reads it aloud.
        app.buttons["voice-mode-open"].tap()
        let agentText = app.staticTexts["voice-agent-text"]
        XCTAssertTrue(agentText.waitForExistence(timeout: 10))
        let spoken = app.descendants(matching: .any)["voice-last-spoken"]
        XCTAssertTrue(waitUntil(timeout: 240) { !spoken.label.isEmpty }, "no turn was read aloud")
        XCTAssertFalse(agentText.label.isEmpty)
        XCTAssertNotEqual(agentText.label, "Waiting for the agent…")
        XCTAssertEqual(spoken.label, agentText.label)

        if audio != nil || transcript != nil {
            // The test audio is dictated after speech ends and sent on silence.
            let sent = app.staticTexts["voice-last-sent"]
            let failure = app.staticTexts["voice-error"]
            XCTAssertTrue(waitUntil(timeout: 120) { sent.exists || failure.exists },
                          "dictated reply was never sent; phase: \(app.descendants(matching: .any)["voice-phase"].label)")
            XCTAssertFalse(failure.exists, "dictation failed: \(failure.label)")

            // The agent answers what was said, and that answer is read too.
            let firstReply = spoken.label
            XCTAssertTrue(waitUntil(timeout: 240) { !spoken.label.isEmpty && spoken.label != firstReply },
                          "the reply to the dictated message was never read aloud")
        }
    }

    /// A fresh simulator introduces its keyboard features (slide to type,
    /// bilingual typing) over the keyboard the first time it appears.
    private static func dismissKeyboardTips(_ app: XCUIApplication) {
        for _ in 0 ..< 3 where app.buttons["Continue"].waitForExistence(timeout: 1.5) {
            app.buttons["Continue"].tap()
        }
    }

    /// Parses "top N of M; coloured K; cols C" from the terminal's value.
    private static func field(_ element: XCUIElement, _ name: String) -> Int {
        let value = element.value as? String ?? ""
        guard let range = value.range(of: name + " ") else { return -1 }
        return Int(value[range.upperBound...].prefix { $0.isNumber }) ?? -1
    }

    private static func coloured(_ element: XCUIElement) -> Int { field(element, "coloured") }
    private static func top(_ element: XCUIElement) -> Int { field(element, "top") }
    private static func total(_ element: XCUIElement) -> Int { field(element, "of") }

    /// The open session's row identifier, read from the drawer before switching away.
    private static func sessionRow(_ app: XCUIApplication, excluding id: String) -> String? {
        app.buttons["show-sessions"].tap()
        let rows = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH 'session-row-'"))
        _ = rows.firstMatch.waitForExistence(timeout: 5)
        let found = rows.allElementsBoundByIndex.first { $0.isSelected }?.identifier
        // Choosing the open session again just closes the drawer.
        if let found { app.descendants(matching: .any)[found].firstMatch.tap() }
        return found
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
