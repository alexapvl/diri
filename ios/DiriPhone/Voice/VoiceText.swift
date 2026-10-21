import Foundation

/// What voice mode reads aloud when an agent finishes a turn.
enum VoiceText {
    /// About forty seconds of speech. Past that a listener mid-set has lost the
    /// thread; the full answer is still on screen and in the terminal.
    static let defaultLimit = 600

    /// The agent's own spoken summary wins; otherwise the last agent turn of
    /// the transcript (the one numbered `turn` when known), made speakable.
    static func utterance(spoken: String?, transcript: [DiriClient.Turn], turn: Int? = nil,
                          limit: Int = defaultLimit) -> String? {
        if let spoken {
            let text = speakable(spoken, limit: limit)
            if !text.isEmpty { return text }
        }
        let agentTurns = transcript.filter {
            $0.isAgent && !$0.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
        let chosen = turn.flatMap { number in agentTurns.last { $0.turn == number } } ?? agentTurns.last
        guard let chosen else { return nil }
        let text = speakable(chosen.text, limit: limit)
        return text.isEmpty ? nil : text
    }

    /// Markdown and code read terribly aloud: fences become a short mention,
    /// links keep their words, markup characters go, and the result is cut at
    /// a sentence boundary.
    static func speakable(_ raw: String, limit: Int = defaultLimit) -> String {
        var text = raw.replacingOccurrences(of: "\r\n", with: "\n")
        text = replace(#"```[\s\S]*?(```|$)"#, in: text, with: "\n(code omitted)\n")
        text = replace(#"!?\[([^\]]*)\]\([^)]*\)"#, in: text, with: "$1")
        text = replace(#"https?://\S+"#, in: text, with: "a link")
        text = replace(#"`([^`]*)`"#, in: text, with: "$1")

        var lines: [String] = []
        for var line in text.components(separatedBy: "\n") {
            line = line.trimmingCharacters(in: .whitespaces)
            // Tables and rules are layout, not language.
            if line.hasPrefix("|") || line.range(of: #"^[-*_=]{3,}$"#, options: .regularExpression) != nil {
                continue
            }
            line = replace(#"^(#{1,6}|>+|[-*+•]|\d+[.)])\s+"#, in: line, with: "")
            line = replace(#"(\*\*|__|\*|~~)"#, in: line, with: "")
            if line.isEmpty { continue }
            // A line without closing punctuation still ends a thought; without
            // a stop, the synthesizer runs a heading into the next sentence.
            if let last = line.last, !".!?:;,…)".contains(last) { line += "." }
            lines.append(line)
        }
        text = lines.joined(separator: " ")
        text = replace(#"\s+"#, in: text, with: " ").trimmingCharacters(in: .whitespaces)
        return truncate(text, limit: limit)
    }

    static func truncate(_ text: String, limit: Int) -> String {
        guard text.count > limit else { return text }
        let head = String(text.prefix(limit))
        let sentenceEnds = [". ", "! ", "? "].compactMap { head.range(of: $0, options: .backwards)?.lowerBound }
        if let end = sentenceEnds.max(), head.distance(from: head.startIndex, to: end) > limit / 3 {
            return String(head[...end])
        }
        if let space = head.lastIndex(of: " ") {
            return String(head[..<space]) + "…"
        }
        return head + "…"
    }

    private static func replace(_ pattern: String, in text: String, with template: String) -> String {
        text.replacingOccurrences(of: pattern, with: template, options: .regularExpression)
    }
}
