import SwiftUI

/// Voice mode as a glass panel over the terminal: a small calm indicator,
/// words at reading size, and capsule controls. The terminal stays visible
/// above it, so you can watch the agent work while you talk to it.
struct VoicePanel: View {
    let sessionID: String
    let onClose: () -> Void
    @Environment(AppModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var controller: VoiceController

    init(sessionID: String, client: DiriClient, onClose: @escaping () -> Void) {
        self.sessionID = sessionID
        self.onClose = onClose
        _controller = State(initialValue: VoiceController(sessionID: sessionID, client: client))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            header
            words
            if let error = controller.error {
                Text(error)
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(Tokens.Ink.danger)
                    .accessibilityIdentifier("voice-error")
            }
            controls
        }
        .padding(.horizontal, 18)
        .padding(.top, 16)
        .padding(.bottom, 14)
        .frame(maxWidth: 560)
        .glass(in: RoundedRectangle(cornerRadius: 30, style: .continuous), tint: Color.black.opacity(0.25))
        // The panel is the button: a tap anywhere talks, sends, or interrupts.
        .contentShape(RoundedRectangle(cornerRadius: 30, style: .continuous))
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
        HStack(spacing: 12) {
            TimelineView(.animation(paused: reduceMotion || indicatorMode == .idle)) { timeline in
                VoiceBars(
                    mode: indicatorMode,
                    level: controller.level,
                    sinceWord: timeline.date.timeIntervalSince(controller.lastWordAt),
                    time: timeline.date.timeIntervalSinceReferenceDate,
                    reduceMotion: reduceMotion
                )
            }
            .frame(width: 60, height: 44)
            .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 2) {
                Text(statusLine)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(statusColor)
                    .accessibilityIdentifier("voice-phase")
                Text(title)
                    .font(.footnote)
                    .foregroundStyle(Tokens.Ink.tertiary)
                    .lineLimit(1)
            }
            Spacer(minLength: 8)
            GlassIconButton(symbol: "xmark", label: "Close voice mode", size: 34, action: onClose)
                .accessibilityIdentifier("voice-close")
        }
    }

    private var indicatorMode: VoiceBars.Mode {
        switch controller.phase {
        case .listening: .listening
        case .speaking: .speaking
        case .sending, .starting: .working
        case .waiting, .idle: agentWorking ? .working : .idle
        }
    }

    // MARK: Words

    private var words: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let context = contextLine {
                Text(context.text)
                    .font(.subheadline)
                    .foregroundStyle(Tokens.Ink.tertiary)
                    .lineLimit(2)
                    .accessibilityIdentifier(context.id)
            }
            // Five lines at reading size; the full text is in the label and
            // in the terminal above.
            main
                .lineLimit(5)
                .fixedSize(horizontal: false, vertical: true)
        }
        .font(.title3.weight(.medium))
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var workingWithoutWords: Bool {
        agentWorking && controller.agentText.isEmpty && controller.lastSent.isEmpty
            && (controller.phase == .idle || controller.phase == .waiting)
    }

    /// Whatever is being said right now, by either side.
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
                    .foregroundStyle(Tokens.Ink.secondary)
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
            result[word].foregroundColor = VoiceBars.ember
        }
        return result
    }

    // MARK: Controls

    private var controls: some View {
        GlassGroup(spacing: 10) {
            HStack(spacing: 10) {
                GlassIconButton(symbol: "arrow.counterclockwise", label: "Repeat", size: 44) {
                    controller.repeatLast()
                }
                .disabled(controller.agentText.isEmpty)
                .opacity(controller.agentText.isEmpty ? 0.45 : 1)
                .accessibilityIdentifier("voice-repeat")

                GlassIconButton(
                    symbol: controller.muted ? "speaker.slash.fill" : "speaker.wave.2.fill",
                    label: controller.muted ? "Unmute" : "Mute", size: 44
                ) {
                    controller.muted.toggle()
                }
                .accessibilityIdentifier("voice-mute")

                Spacer(minLength: 0)

                Button { controller.primaryAction() } label: {
                    HStack(spacing: 8) {
                        Image(systemName: talkSymbol)
                            .font(.system(size: 15, weight: .semibold))
                        Text(talkLabel)
                            .font(.body.weight(.semibold))
                    }
                    .foregroundStyle(Tokens.Ink.background)
                    .padding(.horizontal, 20)
                    .frame(height: 44)
                    .background(Capsule().fill(Tokens.Ink.primary))
                    .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(talkLabel)
                .accessibilityIdentifier("voice-talk")
            }
        }
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
        case .listening: "Send"
        case .speaking: "Interrupt"
        default: "Talk"
        }
    }

    private var talkSymbol: String {
        switch controller.phase {
        case .listening: "arrow.up"
        case .speaking: "hand.raised.fill"
        default: "mic.fill"
        }
    }

    private var statusColor: Color {
        switch controller.phase {
        case .listening: VoiceBars.ember
        default: Tokens.Ink.secondary
        }
    }

    private var statusLine: String {
        switch controller.phase {
        case .starting: "Starting…"
        case .speaking: "Speaking · tap to interrupt"
        case .listening: "Listening · pause to send"
        case .sending: "Sending…"
        case .waiting, .idle:
            switch model.session(sessionID)?.status {
            case .working?, .starting?: "Working…"
            case .needsInput?: "Needs your input"
            case .exited?: "Session ended"
            default: controller.muted ? "Muted · tap to talk" : "Tap to talk"
            }
        }
    }

    private var title: String {
        guard let session = model.session(sessionID) else { return "Voice" }
        let trimmed = session.title.trimmingCharacters(in: .whitespacesAndNewlines)
        let name = trimmed.isEmpty ? (session.cwd as NSString).lastPathComponent : trimmed
        return "\(name) · \(model.computerName(of: session))"
    }
}

/// Five ember bars: still when idle, a slow wave while the agent works,
/// following the voice while listening, and a beat per word while speaking.
struct VoiceBars: View {
    enum Mode { case idle, listening, speaking, working }

    static let ember = Color(f32: 1.0, 0.584, 0.333)

    let mode: Mode
    let level: Double
    let sinceWord: TimeInterval
    let time: TimeInterval
    let reduceMotion: Bool

    private let weights: [Double] = [0.55, 0.8, 1.0, 0.8, 0.55]

    var body: some View {
        HStack(alignment: .center, spacing: 5) {
            ForEach(0 ..< 5, id: \.self) { index in
                Capsule()
                    .fill(color)
                    .frame(width: 6, height: height(index))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var color: Color {
        switch mode {
        case .idle: Tokens.Ink.primary.opacity(0.35)
        case .working: Self.ember.opacity(0.7)
        case .listening, .speaking: Self.ember
        }
    }

    private func height(_ index: Int) -> CGFloat {
        let minimum = 6.0, maximum = 36.0
        guard !reduceMotion else { return mode == .idle ? minimum : 16 * weights[index] + minimum }
        let phase = Double(index) * 0.7
        let amount: Double
        switch mode {
        case .idle:
            amount = 0
        case .working:
            amount = 0.18 + 0.14 * (0.5 + 0.5 * sin(time * 2.4 - phase))
        case .listening:
            amount = 0.12 + min(1, level * 1.6) * (0.75 + 0.25 * sin(time * 9 + phase))
        case .speaking:
            let beat = sinceWord < 1 ? exp(-sinceWord * 6) : 0
            amount = 0.2 + 0.55 * beat * (0.7 + 0.3 * sin(time * 7 + phase))
        }
        return minimum + (maximum - minimum) * amount * weights[index]
    }
}
