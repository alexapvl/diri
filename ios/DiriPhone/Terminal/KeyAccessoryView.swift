import UIKit

/// The row above the software keyboard: the keys a phone keyboard lacks, as
/// quiet glyphs on one glass capsule. No per-key caps — the capsule is the
/// only surface, so the row reads as one control, not fifteen.
@MainActor
final class KeyAccessoryView: UIView {
    private weak var terminal: TerminalView?
    private let glass: UIVisualEffectView
    private let stack = UIStackView()
    private var ctrlButton: UIButton?

    var ctrlLatched = false { didSet { styleCtrl() } }

    private struct Key {
        var title: String?
        var symbol: String?
        var label: String
        var id: String
        var action: (TerminalView) -> Void
    }

    init(terminal: TerminalView) {
        self.terminal = terminal
        if #available(iOS 26.0, *) {
            let effect = UIGlassEffect()
            effect.isInteractive = true
            glass = UIVisualEffectView(effect: effect)
        } else {
            glass = UIVisualEffectView(effect: UIBlurEffect(style: .systemThinMaterialDark))
        }
        super.init(frame: CGRect(x: 0, y: 0, width: 390, height: 54))
        autoresizingMask = .flexibleWidth
        backgroundColor = .clear

        glass.translatesAutoresizingMaskIntoConstraints = false
        glass.clipsToBounds = true
        glass.layer.cornerRadius = 21
        glass.layer.cornerCurve = .continuous
        addSubview(glass)

        stack.axis = .horizontal
        stack.distribution = .equalSpacing
        stack.alignment = .center
        stack.translatesAutoresizingMaskIntoConstraints = false
        glass.contentView.addSubview(stack)

        NSLayoutConstraint.activate([
            glass.leadingAnchor.constraint(equalTo: safeAreaLayoutGuide.leadingAnchor, constant: 10),
            glass.trailingAnchor.constraint(equalTo: safeAreaLayoutGuide.trailingAnchor, constant: -10),
            glass.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            glass.heightAnchor.constraint(equalToConstant: 42),
            stack.leadingAnchor.constraint(equalTo: glass.contentView.leadingAnchor, constant: 8),
            stack.trailingAnchor.constraint(equalTo: glass.contentView.trailingAnchor, constant: -8),
            stack.topAnchor.constraint(equalTo: glass.contentView.topAnchor),
            stack.bottomAnchor.constraint(equalTo: glass.contentView.bottomAnchor),
        ])

        let keys: [Key] = [
            Key(title: "esc", label: "Escape", id: "key-esc") { $0.press(.escape) },
            Key(symbol: "arrow.right.to.line", label: "Tab", id: "key-tab") { $0.press(.tab) },
            Key(title: "ctrl", label: "Control", id: "key-ctrl") { $0.toggleCtrl() },
            Key(symbol: "arrow.left", label: "Left", id: "key-left") { $0.press(.left) },
            Key(symbol: "arrow.down", label: "Down", id: "key-down") { $0.press(.down) },
            Key(symbol: "arrow.up", label: "Up", id: "key-up") { $0.press(.up) },
            Key(symbol: "arrow.right", label: "Right", id: "key-right") { $0.press(.right) },
            Key(title: "/", label: "Slash", id: "key-slash") { $0.press(.text("/")) },
            Key(title: "|", label: "Pipe", id: "key-pipe") { $0.press(.text("|")) },
            Key(symbol: "keyboard.chevron.compact.down", label: "Hide keyboard", id: "key-hide") { $0.resignFirstResponder() },
        ]
        for key in keys { stack.addArrangedSubview(button(for: key)) }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override var intrinsicContentSize: CGSize { CGSize(width: UIView.noIntrinsicMetric, height: 54) }


    private func button(for key: Key) -> UIButton {
        var config = UIButton.Configuration.plain()
        config.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 6, bottom: 8, trailing: 6)
        config.baseForegroundColor = UIColor.white.withAlphaComponent(0.82)
        if let symbol = key.symbol {
            config.image = UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: 14, weight: .medium))
        }
        if let title = key.title {
            var attributed = AttributedString(title)
            attributed.font = UIFont.monospacedSystemFont(ofSize: 15, weight: .medium)
            config.attributedTitle = attributed
        }
        let button = UIButton(configuration: config)
        button.accessibilityLabel = key.label
        button.accessibilityIdentifier = key.id
        button.addAction(UIAction { [weak self] _ in
            guard let terminal = self?.terminal else { return }
            UIImpactFeedbackGenerator(style: .soft).impactOccurred(intensity: 0.6)
            key.action(terminal)
        }, for: .primaryActionTriggered)
        button.widthAnchor.constraint(greaterThanOrEqualToConstant: 30).isActive = true
        if key.id == "key-ctrl" { ctrlButton = button }
        return button
    }

    private func styleCtrl() {
        guard let ctrlButton, var config = ctrlButton.configuration else { return }
        config.baseForegroundColor = ctrlLatched ? UIColor(Tokens.Ink.clay) : UIColor.white.withAlphaComponent(0.82)
        var attributed = AttributedString("ctrl")
        attributed.font = UIFont.monospacedSystemFont(ofSize: 15, weight: ctrlLatched ? .bold : .medium)
        config.attributedTitle = attributed
        ctrlButton.configuration = config
        ctrlButton.accessibilityValue = ctrlLatched ? "On" : "Off"
    }
}
