import XCTest
@testable import DiriPhone

final class VoiceTextTests: XCTestCase {
    private func turn(_ role: String, _ text: String, _ number: Int? = nil) -> DiriClient.Turn {
        .init(role: role, text: text, turn: number)
    }

    func testSpokenSummaryWins() {
        let text = VoiceText.utterance(spoken: "Tests pass. Ready to merge.",
                                       transcript: [turn("agent", "A long answer")])
        XCTAssertEqual(text, "Tests pass. Ready to merge.")
    }

    func testBlankSpokenFallsBackToLastAgentTurn() {
        let transcript = [turn("agent", "First."), turn("user", "and?"), turn("agent", "Second."), turn("user", "ok")]
        XCTAssertEqual(VoiceText.utterance(spoken: "  \n", transcript: transcript), "Second.")
        XCTAssertEqual(VoiceText.utterance(spoken: nil, transcript: transcript), "Second.")
    }

    func testNumberedTurnIsPreferredAndEmptyAgentTurnsSkipped() {
        let transcript = [turn("agent", "Turn two.", 2), turn("agent", "Turn three.", 3), turn("agent", "   ", 4)]
        XCTAssertEqual(VoiceText.utterance(spoken: nil, transcript: transcript, turn: 2), "Turn two.")
        XCTAssertEqual(VoiceText.utterance(spoken: nil, transcript: transcript, turn: 9), "Turn three.")
        XCTAssertNil(VoiceText.utterance(spoken: nil, transcript: [turn("user", "hi")]))
    }

    func testMarkdownAndCodeAreMadeSpeakable() {
        let raw = """
        ## Summary
        I fixed **the bug** in `parser.rs`. See [the PR](https://github.com/x/y/pull/1).

        ```rust
        fn main() {}
        ```

        | a | b |
        |---|---|
        - Added tests
        - Docs at https://example.com/docs
        """
        XCTAssertEqual(
            VoiceText.speakable(raw),
            "Summary. I fixed the bug in parser.rs. See the PR. (code omitted) Added tests. Docs at a link."
        )
    }

    func testLongTextIsCutAtASentence() {
        let sentence = "This sentence is exactly forty-six characters. "
        let long = String(repeating: sentence, count: 30)
        let cut = VoiceText.speakable(long, limit: 200)
        XCTAssertLessThanOrEqual(cut.count, 200)
        XCTAssertTrue(cut.hasSuffix("characters."), cut)

        let words = String(repeating: "word ", count: 100)
        let cutWords = VoiceText.truncate(words.trimmingCharacters(in: .whitespaces), limit: 50)
        XCTAssertTrue(cutWords.hasSuffix("word…"), cutWords)
    }
}
