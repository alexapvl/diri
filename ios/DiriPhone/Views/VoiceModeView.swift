import SwiftUI

/// Gym mode: one huge target, readable at arm's length, usable with the
/// screen locked and AirPods in.
struct VoiceModeView: View {
    let sessionID: String
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var controller: VoiceController

    init(sessionID: String, client: DiriClient) {
        self.sessionID = sessionID
        _controller = State(initialValue: VoiceController(sessionID: sessionID, client: client))
    }

    var body: some View {
        VStack(spacing: 20) {
            header
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Text(controller.agentText.isEmpty ? "Waiting for the agent…" : controller.agentText)
                        .font(.system(size: 22, weight: .regular))
                        .foregroundStyle(controller.agentText.isEmpty ? Tokens.Ink.tertiary : Tokens.Ink.primary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .accessibilityIdentifier("voice-agent-text")
                    if !controller.heard.isEmpty {
                        Text(controller.heard)
                            .font(.system(size: 20, weight: .medium))
                            .foregroundStyle(Tokens.Ink.clay)
                            .accessibilityIdentifier("voice-heard")
                    } else if !controller.lastSent.isEmpty {
                        Text("You: \(controller.lastSent)")
                            .font(.system(size: 17))
                            .foregroundStyle(Tokens.Ink.secondary)
                            .accessibilityIdentifier("voice-last-sent")
                    }
                }
                .padding(.horizontal, 24)
            }
            if let error = controller.error {
                Text(error)
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(Tokens.Ink.danger)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 24)
                    .accessibilityIdentifier("voice-error")
            }
            Text(statusLine)
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(Tokens.Ink.secondary)
                .accessibilityIdentifier("voice-phase")
            talkButton
            HStack(spacing: 16) {
                bigSecondary("Repeat", symbol: "arrow.counterclockwise", id: "voice-repeat") {
                    controller.repeatLast()
                }
                .disabled(controller.agentText.isEmpty)
                bigSecondary(controller.muted ? "Unmute" : "Mute",
                             symbol: controller.muted ? "speaker.slash.fill" : "speaker.wave.2.fill",
                             id: "voice-mute") {
                    controller.muted.toggle()
                }
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 12)
        }
        .background(Tokens.Ink.background.ignoresSafeArea())
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

    private var header: some View {
        HStack {
            Button { dismiss() } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 20, weight: .semibold))
                    .frame(width: 52, height: 52)
                    .background(Circle().fill(Tokens.Ink.floatingSurface))
            }
            .accessibilityLabel("Close voice mode")
            .accessibilityIdentifier("voice-close")
            Spacer()
            Text(title)
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(Tokens.Ink.primary)
                .lineLimit(1)
            Spacer()
            Color.clear.frame(width: 52, height: 52)
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
    }

    private var talkButton: some View {
        Button { controller.primaryAction() } label: {
            ZStack {
                Circle()
                    .fill(controller.phase == .listening ? Tokens.Ink.danger : Tokens.Ink.clay)
                if controller.phase == .sending || controller.phase == .starting {
                    ProgressView().controlSize(.large).tint(Tokens.Ink.background)
                } else {
                    Image(systemName: talkSymbol)
                        .font(.system(size: 72, weight: .bold))
                        .foregroundStyle(Tokens.Ink.background)
                }
            }
            .frame(width: 220, height: 220)
            .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .disabled(controller.phase == .sending || controller.phase == .starting)
        .accessibilityLabel(talkLabel)
        .accessibilityIdentifier("voice-talk")
    }

    private func bigSecondary(_ label: String, symbol: String, id: String,
                              action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(label, systemImage: symbol)
                .font(.system(size: 19, weight: .semibold))
                .frame(maxWidth: .infinity, minHeight: 64)
                .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Tokens.Ink.floatingSurface))
                .foregroundStyle(Tokens.Ink.primary)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(id)
    }

    private var talkSymbol: String {
        switch controller.phase {
        case .listening: "paperplane.fill"
        case .speaking: "hand.raised.fill"
        default: "mic.fill"
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
        case .speaking: "Reading aloud"
        case .listening: "Listening — pause to send"
        case .sending: "Sending…"
        case .waiting, .idle:
            switch model.session(sessionID)?.status {
            case .working?, .starting?: "Agent is working"
            case .needsInput?: "Agent needs input"
            case .exited?: "Session ended"
            default: controller.muted ? "Muted — tap to talk" : "Tap to talk"
            }
        }
    }

    private var title: String {
        guard let session = model.session(sessionID) else { return "Voice" }
        let trimmed = session.title.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? (session.cwd as NSString).lastPathComponent : trimmed
    }
}
