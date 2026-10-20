import SwiftUI

/// Gym mode: glanced at from a bench between sets, so it has to read at
/// arm's length. One orb carries the state through motion, one block of large
/// type carries the words, and the whole screen is the button.
struct VoiceModeView: View {
    let sessionID: String
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var controller: VoiceController

    init(sessionID: String, client: DiriClient) {
        self.sessionID = sessionID
        _controller = State(initialValue: VoiceController(sessionID: sessionID, client: client))
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            // Fixed, not a spacer: the orb must not move as the words below
            // it grow and shrink.
            orb
                .padding(.top, 72)
            Text(statusLine)
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(Tokens.Ink.tertiary)
                .padding(.top, 28)
                .opacity(workingWithoutWords ? 0 : 1)
                .accessibilityIdentifier("voice-phase")
            words
                .padding(.top, 14)
            Spacer(minLength: 16)
            if let error = controller.error {
                Text(error)
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(Tokens.Ink.danger)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
                    .padding(.bottom, 8)
                    .accessibilityIdentifier("voice-error")
            }
            controls
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Tokens.Ink.background.ignoresSafeArea())
        // Anywhere is the button: a sweaty thumb should not have to aim.
        .contentShape(Rectangle())
        .onTapGesture { controller.primaryAction() }
        .sensoryFeedback(.success, trigger: controller.lastSent)
        .sensoryFeedback(.impact(weight: .light), trigger: controller.phase == .listening)
        .overlay(alignment: .topLeading) {
            // What the synthesizer was last handed, for UI tests.
            Color.clear
                .frame(width: 1, height: 1)
                .accessibilityElement()
                .accessibilityLabel(controller.lastSpoken)
                .accessibilityIdentifier("voice-last-spoken")
        }
        .task { await controller.start() }
        .onDisappear { controller.stop() }
    }

    // MARK: Header

    private var header: some View {
        ZStack {
            VStack(spacing: 2) {
                Text(title)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Tokens.Ink.secondary)
                    .lineLimit(1)
                if let computer {
                    Text(computer)
                        .font(.system(size: 13))
                        .foregroundStyle(Tokens.Ink.tertiary)
                        .lineLimit(1)
                }
            }
            .padding(.horizontal, 64)
            HStack {
                Button { dismiss() } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(Tokens.Ink.secondary)
                        .frame(width: 44, height: 44)
                        .contentShape(Rectangle())
                }
                .accessibilityLabel("Close voice mode")
                .accessibilityIdentifier("voice-close")
                Spacer()
            }
        }
        .padding(.horizontal, 12)
        .padding(.top, 4)
    }

    // MARK: Orb

    private var orb: some View {
        Button { controller.primaryAction() } label: {
            TimelineView(.animation(paused: reduceMotion || !orbMoves)) { timeline in
                EmberOrb(
                    mode: orbMode,
                    level: controller.level,
                    sinceWord: timeline.date.timeIntervalSince(controller.lastWordAt),
                    time: timeline.date.timeIntervalSinceReferenceDate,
                    reduceMotion: reduceMotion
                )
            }
            .frame(width: 200, height: 200)
            .contentShape(Circle())
        }
        .buttonStyle(.plain)
        // Not `.disabled`: that greys the orb, and taps while sending are
        // already ignored by `primaryAction`.
        .accessibilityLabel(talkLabel)
        .accessibilityIdentifier("voice-talk")
    }

    private var orbMode: EmberOrb.Mode {
        switch controller.phase {
        case .listening: .listening
        case .speaking: .speaking
        case .sending, .starting: .working
        case .waiting, .idle: agentWorking ? .working : .idle
        }
    }

    private var orbMoves: Bool { orbMode != .idle }

    // MARK: Words

    @ViewBuilder
    private var words: some View {
        VStack(spacing: 14) {
            if let context = contextLine {
                Text(context.text)
                    .font(.system(size: 17))
                    .foregroundStyle(Tokens.Ink.tertiary)
                    .multilineTextAlignment(.center)
                    .lineLimit(2)
                    .accessibilityIdentifier(context.id)
            }
            // Largest type that fits; only a long reply scrolls. Sized to its
            // content so a short line sits with the orb, not atop a void.
            ViewThatFits(in: .vertical) {
                styled(main, size: 30)
                styled(main, size: 24)
                ScrollView { styled(main, size: 24) }
                    .scrollBounceBehavior(.basedOnSize)
            }
            .frame(maxHeight: 340, alignment: .top)
        }
        .padding(.horizontal, 28)
    }

    private func styled(_ content: some View, size: CGFloat) -> some View {
        content
            .font(.system(size: size, weight: .semibold))
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity)
            .fixedSize(horizontal: false, vertical: true)
    }

    /// The agent is on its first turn and nobody has said anything yet: the
    /// orb's shimmer is the whole message.
    private var workingWithoutWords: Bool {
        agentWorking && controller.agentText.isEmpty && controller.lastSent.isEmpty
            && (controller.phase == .idle || controller.phase == .waiting)
    }

    /// The one big block: whatever is being said right now, by either side.
    @ViewBuilder
    private var main: some View {
        switch controller.phase {
        case .listening, .sending:
            Text(controller.heard.isEmpty ? "Listening…" : controller.heard)
                .foregroundStyle(controller.heard.isEmpty ? Tokens.Ink.tertiary : Tokens.Ink.primary)
                .accessibilityIdentifier("voice-heard")
        case .speaking:
            Text(readingAloud)
                .accessibilityLabel(controller.agentText)
                .accessibilityIdentifier("voice-agent-text")
        case .waiting, .idle, .starting:
            if agentWorking, !controller.lastSent.isEmpty {
                Text(controller.lastSent)
                    .foregroundStyle(Tokens.Ink.tertiary)
                    .accessibilityIdentifier("voice-last-sent")
            } else if workingWithoutWords {
                Text("Working on it…")
                    .foregroundStyle(Tokens.Ink.tertiary)
                    .accessibilityIdentifier("voice-agent-text")
            } else {
                Text(controller.agentText.isEmpty ? "Waiting for the agent…" : controller.agentText)
                    .foregroundStyle(controller.agentText.isEmpty ? Tokens.Ink.tertiary : Tokens.Ink.primary)
                    .accessibilityIdentifier("voice-agent-text")
            }
        }
    }

    /// One quiet line above the big block, so the other side of the
    /// conversation is never lost.
    private var contextLine: (text: String, id: String)? {
        switch controller.phase {
        case .listening, .sending:
            controller.agentText.isEmpty ? nil : (controller.agentText, "voice-agent-text")
        case .speaking:
            nil
        case .waiting, .idle, .starting:
            if agentWorking, !controller.lastSent.isEmpty {
                controller.agentText.isEmpty ? nil : (controller.agentText, "voice-agent-text")
            } else if !controller.lastSent.isEmpty {
                (controller.lastSent, "voice-last-sent")
            } else {
                nil
            }
        }
    }

    /// Words already read in white, the one being read in ember, the rest dim.
    private var readingAloud: AttributedString {
        let text = controller.agentText
        var result = AttributedString(text)
        result.foregroundColor = Tokens.Ink.tertiary
        guard let range = controller.spokenRange,
              range.location != NSNotFound, NSMaxRange(range) <= (text as NSString).length
        else { return result }
        if let done = Range(NSRange(location: 0, length: range.location), in: result) {
            result[done].foregroundColor = Tokens.Ink.primary
        }
        if let word = Range(range, in: result) {
            result[word].foregroundColor = EmberOrb.ember
        }
        return result
    }

    // MARK: Controls

    private var controls: some View {
        HStack {
            quiet(symbol: "arrow.counterclockwise", label: "Repeat", id: "voice-repeat") {
                controller.repeatLast()
            }
            .disabled(controller.agentText.isEmpty)
            Spacer()
            quiet(symbol: controller.muted ? "speaker.slash.fill" : "speaker.wave.2.fill",
                  label: controller.muted ? "Unmute" : "Mute", id: "voice-mute") {
                controller.muted.toggle()
            }
        }
        .padding(.horizontal, 28)
        .padding(.bottom, 8)
    }

    private func quiet(symbol: String, label: String, id: String,
                       action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 24, weight: .medium))
                .foregroundStyle(Tokens.Ink.secondary)
                .frame(width: 64, height: 64)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }

    // MARK: Copy

    private var agentWorking: Bool {
        switch model.session(sessionID)?.status {
        case .working?, .starting?: true
        default: false
        }
    }

    private var talkLabel: String {
        switch controller.phase {
        case .listening: "Send now"
        case .speaking: "Interrupt and talk"
        default: "Talk"
        }
    }

    private var statusLine: String {
        switch controller.phase {
        case .starting: "Starting…"
        case .speaking: "Tap to interrupt"
        case .listening: "Pause to send · tap to send now"
        case .sending: "Sending…"
        case .waiting, .idle:
            switch model.session(sessionID)?.status {
            case .working?, .starting?: "Working…"
            case .needsInput?: "Needs your input"
            case .exited?: "Session ended"
            default: controller.muted ? "Muted · tap anywhere to talk" : "Tap anywhere to talk"
            }
        }
    }

    private var title: String {
        guard let session = model.session(sessionID) else { return "Voice" }
        let trimmed = session.title.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? (session.cwd as NSString).lastPathComponent : trimmed
    }

    private var computer: String? {
        model.session(sessionID)?.gateway ?? model.liveGateway?.name
    }
}

/// Diri's ember: a warm core with a soft glow, the app icon's light source.
/// State is carried by motion alone, never by a swapped glyph.
struct EmberOrb: View {
    enum Mode { case idle, listening, speaking, working }

    static let ember = Color(f32: 1.0, 0.584, 0.333)
    static let core = Color(f32: 1.0, 0.792, 0.584)

    let mode: Mode
    let level: Double
    let sinceWord: TimeInterval
    let time: TimeInterval
    let reduceMotion: Bool

    var body: some View {
        let scale = reduceMotion ? 1 : self.scale
        ZStack {
            // Glow.
            Circle()
                .fill(RadialGradient(
                    colors: [Self.ember.opacity(glow), Self.ember.opacity(0)],
                    center: .center, startRadius: 30, endRadius: 100
                ))
                .scaleEffect(scale * 1.05)
            // Body.
            Circle()
                .fill(RadialGradient(
                    colors: [Self.core, Self.ember, Self.ember.opacity(0.92)],
                    center: .center, startRadius: 0, endRadius: 58
                ))
                .frame(width: 112, height: 112)
                .overlay {
                    if mode == .working, !reduceMotion {
                        // A slow highlight travelling round the rim.
                        Circle()
                            .strokeBorder(
                                AngularGradient(
                                    stops: [
                                        .init(color: Self.core.opacity(0), location: 0),
                                        .init(color: Self.core.opacity(0), location: 0.7),
                                        .init(color: Self.core, location: 0.9),
                                        .init(color: Self.core.opacity(0), location: 1),
                                    ],
                                    center: .center,
                                    angle: .degrees(time.truncatingRemainder(dividingBy: 2.4) / 2.4 * 360)
                                ),
                                lineWidth: 6
                            )
                            .blur(radius: 3)
                            .clipShape(Circle())
                    }
                }
                .scaleEffect(scale)
                .opacity(mode == .idle ? 0.78 : 1)
        }
    }

    private var scale: Double {
        switch mode {
        case .idle:
            return 1
        case .listening:
            // Breathes on its own so it never looks frozen, and swells with
            // the voice on top of that.
            return 1 + 0.03 * sin(time * 2.2) + 0.22 * level
        case .speaking:
            // A beat on every word, decaying before the next.
            let beat = sinceWord < 1 ? 0.09 * exp(-sinceWord * 7) : 0
            return 1.02 + 0.02 * sin(time * 3) + beat
        case .working:
            return 0.96 + 0.02 * sin(time * 1.4)
        }
    }

    private var glow: Double {
        switch mode {
        case .idle: 0.18
        case .listening: 0.30 + 0.35 * level
        case .speaking: 0.38
        case .working: 0.22 + 0.06 * sin(time * 1.4)
        }
    }
}
