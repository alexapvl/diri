import Foundation

/// A key as a terminal sees it, before modifiers are folded in.
enum TermKey: Hashable, Sendable {
    case text(String)
    case enter, tab, escape, backspace, delete
    case up, down, left, right
    case home, end, pageUp, pageDown
}

struct KeyModifiers: OptionSet, Hashable, Sendable {
    let rawValue: UInt8
    static let shift = KeyModifiers(rawValue: 1)
    static let alt = KeyModifiers(rawValue: 2)
    static let ctrl = KeyModifiers(rawValue: 4)
}

/// Turns keys into the bytes an xterm-compatible program expects.
enum KeyEncoder {
    static let esc: UInt8 = 0x1B

    static func encode(_ key: TermKey, modifiers: KeyModifiers = [], appCursor: Bool = false) -> [UInt8] {
        switch key {
        case let .text(text):
            return encodeText(text, modifiers: modifiers)
        case .enter:
            return withAlt([0x0D], modifiers)
        case .tab:
            return modifiers.contains(.shift) ? [esc] + Array("[Z".utf8) : withAlt([0x09], modifiers)
        case .escape:
            return [esc]
        case .backspace:
            return withAlt([modifiers.contains(.ctrl) ? 0x08 : 0x7F], modifiers)
        case .delete:
            return tilde(3, modifiers)
        case .up: return cursor("A", modifiers, appCursor)
        case .down: return cursor("B", modifiers, appCursor)
        case .right: return cursor("C", modifiers, appCursor)
        case .left: return cursor("D", modifiers, appCursor)
        case .home: return cursor("H", modifiers, appCursor)
        case .end: return cursor("F", modifiers, appCursor)
        case .pageUp: return tilde(5, modifiers)
        case .pageDown: return tilde(6, modifiers)
        }
    }

    /// Typed text. Ctrl folds a letter or one of `@[\]^_?` and space to its
    /// control code; Alt prefixes ESC (the meta convention every TUI reads).
    static func encodeText(_ text: String, modifiers: KeyModifiers = []) -> [UInt8] {
        // A soft keyboard's return arrives as "\n"; a terminal sends CR.
        let normalized = text.replacingOccurrences(of: "\r\n", with: "\r").replacingOccurrences(of: "\n", with: "\r")
        if modifiers.contains(.ctrl), normalized.count == 1, let control = controlCode(normalized) {
            return withAlt([control], modifiers)
        }
        return withAlt(Array(normalized.utf8), modifiers)
    }

    static func controlCode(_ character: String) -> UInt8? {
        guard let scalar = character.unicodeScalars.first, character.unicodeScalars.count == 1,
              scalar.isASCII else { return nil }
        let value = UInt8(scalar.value)
        switch value {
        case UInt8(ascii: "a") ... UInt8(ascii: "z"): return value - UInt8(ascii: "a") + 1
        case UInt8(ascii: "@") ... UInt8(ascii: "_"): return value & 0x1F // @ A-Z [ \ ] ^ _
        case UInt8(ascii: " "), UInt8(ascii: "2"): return 0x00
        case UInt8(ascii: "3"): return esc
        case UInt8(ascii: "4"): return 0x1C
        case UInt8(ascii: "5"): return 0x1D
        case UInt8(ascii: "6"): return 0x1E
        case UInt8(ascii: "7"), UInt8(ascii: "/"), UInt8(ascii: "-"): return 0x1F
        case UInt8(ascii: "8"), UInt8(ascii: "?"): return 0x7F
        default: return nil
        }
    }

    /// Pasted text, wrapped in bracketed-paste markers when the program asked
    /// for them. A pasted end marker is stripped so the paste cannot end itself
    /// early and smuggle the rest in as typed keys.
    static func paste(_ text: String, bracketed: Bool) -> [UInt8] {
        var body = text.replacingOccurrences(of: "\r\n", with: "\r").replacingOccurrences(of: "\n", with: "\r")
        guard bracketed else { return Array(body.utf8) }
        body = body.replacingOccurrences(of: "\u{1B}[201~", with: "")
        return [esc] + Array("[200~".utf8) + Array(body.utf8) + [esc] + Array("[201~".utf8)
    }

    // MARK: - Helpers

    /// xterm's modifier parameter: 1 + shift + 2·alt + 4·ctrl.
    private static func parameter(_ modifiers: KeyModifiers) -> Int {
        1 + (modifiers.contains(.shift) ? 1 : 0) + (modifiers.contains(.alt) ? 2 : 0) + (modifiers.contains(.ctrl) ? 4 : 0)
    }

    private static func cursor(_ final: Character, _ modifiers: KeyModifiers, _ appCursor: Bool) -> [UInt8] {
        let p = parameter(modifiers)
        if p > 1 { return [esc] + Array("[1;\(p)\(final)".utf8) }
        return [esc] + Array((appCursor ? "O" : "[").utf8) + Array(String(final).utf8)
    }

    private static func tilde(_ code: Int, _ modifiers: KeyModifiers) -> [UInt8] {
        let p = parameter(modifiers)
        return [esc] + Array((p > 1 ? "[\(code);\(p)~" : "[\(code)~").utf8)
    }

    private static func withAlt(_ bytes: [UInt8], _ modifiers: KeyModifiers) -> [UInt8] {
        modifiers.contains(.alt) ? [esc] + bytes : bytes
    }
}
