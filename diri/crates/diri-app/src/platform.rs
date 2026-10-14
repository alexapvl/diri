//! User-facing desktop vocabulary that genuinely varies by operating system.

pub fn local_machine_label() -> &'static str {
    crate::i18n::t(if cfg!(target_os = "macos") {
        "platform.this_mac"
    } else if cfg!(windows) {
        "platform.this_pc"
    } else {
        "platform.this_computer"
    })
}

pub fn local_machine_label_lowercase() -> &'static str {
    crate::i18n::t(if cfg!(target_os = "macos") {
        "platform.this_mac_lowercase"
    } else if cfg!(windows) {
        "platform.this_pc_lowercase"
    } else {
        "platform.this_computer_lowercase"
    })
}

/// The button that opens a folder in the system file manager.
pub fn reveal_in_file_manager_label() -> &'static str {
    crate::i18n::t(if cfg!(target_os = "macos") {
        "settings.privacy.show_in_finder"
    } else if cfg!(windows) {
        "platform.show_in_file_explorer"
    } else {
        "platform.show_in_folder"
    })
}

/// Leaving the app: macOS quits, Windows exits.
pub fn quit_label() -> &'static str {
    crate::i18n::t(if cfg!(windows) {
        "command.exit.title"
    } else {
        "command.quit.title"
    })
}

/// Whether the modifier diri's shortcuts, multi-select and link clicks use is
/// down: ⌘ on macOS, Ctrl elsewhere (`commands::linux_chord`). GPUI's
/// `platform` modifier is the Windows key on Windows, which opens Start.
pub fn shortcut_modifier(modifiers: &gpui::Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.platform
    } else {
        modifiers.control
    }
}
