import SwiftUI

/// Liquid Glass with a material fallback for iOS 17–25. Every floating
/// surface in the app goes through here, so they all share one material.
extension View {
    @ViewBuilder
    func glass<S: Shape>(in shape: S, interactive: Bool = false, tint: Color? = nil) -> some View {
        if #available(iOS 26.0, *) {
            self.glassEffect(Self.makeGlass(interactive: interactive, tint: tint), in: shape)
        } else {
            self
                .background(shape.fill(.ultraThinMaterial))
                .background(shape.fill((tint ?? .clear)))
                .overlay(shape.stroke(Tokens.Ink.floatingStroke, lineWidth: 1))
        }
    }

    @available(iOS 26.0, *)
    private static func makeGlass(interactive: Bool, tint: Color?) -> Glass {
        var glass = Glass.regular
        if let tint { glass = glass.tint(tint) }
        if interactive { glass = glass.interactive() }
        return glass
    }
}

/// `GlassEffectContainer` where available, so neighbouring capsules blend
/// into one another instead of stacking two refractions.
struct GlassGroup<Content: View>: View {
    var spacing: CGFloat = 8
    @ViewBuilder var content: Content

    var body: some View {
        if #available(iOS 26.0, *) {
            GlassEffectContainer(spacing: spacing) { content }
        } else {
            content
        }
    }
}

/// A round glass button carrying one SF Symbol.
struct GlassIconButton: View {
    let symbol: String
    let label: String
    var size: CGFloat = 40
    var tint: Color = Tokens.Ink.primary.opacity(0.88)
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size * 0.38, weight: .semibold))
                .foregroundStyle(tint)
                .frame(width: size, height: size)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .glass(in: Circle(), interactive: true)
        .accessibilityLabel(label)
    }
}
