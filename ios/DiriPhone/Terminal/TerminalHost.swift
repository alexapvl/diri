import SwiftUI
import UIKit

/// Keeps each opened session's terminal alive — its stream, its scrollback
/// cache and its view — so switching away and back lands on the same scroll
/// position with the screen already drawn. Only the session on screen
/// streams; the others keep their cache and reconnect when shown.
@MainActor
final class TerminalRegistry {
    static let shared = TerminalRegistry()

    private var sessions: [String: TerminalSession] = [:]
    private var views: [String: TerminalView] = [:]
    private var order: [String] = []
    /// Terminals kept warm. Each is a few MB of scrollback at most.
    private let keep = 8

    func session(for id: String, client: DiriClient) -> TerminalSession {
        if let existing = sessions[id] { return existing }
        let session = TerminalSession(sessionID: id, client: client)
        sessions[id] = session
        return session
    }

    func view(for session: TerminalSession) -> TerminalView {
        let id = session.sessionID
        touch(id)
        if let existing = views[id] { return existing }
        let view = TerminalView(session: session)
        views[id] = view
        return view
    }

    func existingView(_ id: String) -> TerminalView? { views[id] }

    /// Makes `id` the live terminal: it streams, the others pause.
    func activate(_ id: String) {
        for (other, session) in sessions where other != id { session.stop() }
        sessions[id]?.start()
    }

    func deactivateAll() {
        for session in sessions.values { session.stop() }
    }

    private func touch(_ id: String) {
        order.removeAll { $0 == id }
        order.append(id)
        while order.count > keep {
            let evicted = order.removeFirst()
            sessions.removeValue(forKey: evicted)?.stop()
            views.removeValue(forKey: evicted)
        }
    }
}

/// Hosts a session's `TerminalView` in SwiftUI.
struct TerminalHost: UIViewRepresentable {
    let session: TerminalSession
    var insets: EdgeInsets
    var onCommand: (TerminalView.Command) -> Void = { _ in }
    var onFocus: (Bool) -> Void = { _ in }

    func makeUIView(context: Context) -> TerminalContainer {
        TerminalContainer()
    }

    func updateUIView(_ container: TerminalContainer, context: Context) {
        let view = TerminalRegistry.shared.view(for: session)
        container.host(view)
        view.onCommand = onCommand
        view.onFocus = onFocus
        view.chromeInsets = UIEdgeInsets(top: insets.top, left: insets.leading, bottom: insets.bottom, right: insets.trailing)
    }

    static func dismantleUIView(_ container: TerminalContainer, coordinator: ()) {
        container.host(nil)
    }
}

/// A plain container so the cached terminal view can move between SwiftUI
/// hosts without SwiftUI owning (and discarding) it.
final class TerminalContainer: UIView {
    private(set) weak var hosted: TerminalView?

    func host(_ view: TerminalView?) {
        guard hosted !== view else { return }
        if let hosted, hosted.superview === self {
            if hosted.isFirstResponder { hosted.resignFirstResponder() }
            hosted.removeFromSuperview()
        }
        hosted = view
        guard let view else { return }
        view.removeFromSuperview()
        view.frame = bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        addSubview(view)
    }
}
