//! Settings > Voice, the parts beyond the toggles: the speech
//! command a "Custom command" voice pipes readings to, what takes the spoken
//! answer (Diri, Wispr Flow, or any dictation app's shortcut), and that
//! shortcut's recorder.
use super::*;
use crate::store::{DictationShortcut, TalkAnswer};
use gpui::{Modifiers, Subscription};

#[derive(Default)]
pub(super) struct VoiceFieldsState {
    command: QueryEditor,
    command_active: bool,
    recording_shortcut: bool,
    /// Swallows key bindings while a shortcut is being recorded, so ⌘K is
    /// captured instead of opening the palette.
    recording_guard: Option<Subscription>,
    gesture: DictationGesture,
}

const ANSWER_OPTIONS: [TalkAnswer; 3] = [TalkAnswer::Diri, TalkAnswer::WisprFlow, TalkAnswer::Shortcut];

fn tabs_label(all: bool) -> &'static str {
    t(if all {
        "settings.general.talk_tabs_all"
    } else {
        "settings.general.talk_tabs_picked"
    })
}

fn answer_label(answer: TalkAnswer) -> &'static str {
    match answer {
        TalkAnswer::Diri => t("settings.general.talk_answer_diri"),
        TalkAnswer::WisprFlow => t("settings.general.talk_answer_wispr"),
        TalkAnswer::Shortcut => t("settings.general.talk_answer_shortcut"),
    }
}

impl UtilitySurfaces {
    pub(super) fn deactivate_voice_fields(&mut self) {
        self.voice_fields.command_active = false;
        self.stop_dictation_recording();
    }

    pub(crate) fn is_recording_dictation_shortcut(&self) -> bool {
        self.voice_fields.recording_shortcut
    }

    /// Typing into the speech command, or recording the dictation shortcut.
    pub(super) fn handle_voice_fields_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.surface != Surface::Settings || self.settings_tab != SettingsTab::Voice {
            return false;
        }
        if self.voice_fields.recording_shortcut {
            if !event.is_held {
                let (key_code, flags) = physical_key();
                let outcome =
                    self.voice_fields
                        .gesture
                        .on_key(&event.keystroke.key, event.keystroke.modifiers, key_code, flags);
                self.apply_dictation_outcome(outcome, cx);
            }
            cx.stop_propagation();
            cx.notify();
            return true;
        }
        if !self.voice_fields.command_active {
            return false;
        }
        let key = &event.keystroke;
        match key.key.as_str() {
            "escape" | "enter" | "tab" => self.voice_fields.command_active = false,
            _ => {
                let Some(edit) = query_editor::edit_for(key) else {
                    return false;
                };
                let field = &mut self.voice_fields.command;
                match edit {
                    Edit::Local(local) => {
                        field.apply(local);
                    }
                    Edit::Clipboard(ClipboardEdit::Copy) => {
                        query_editor::copy_selection(field, cx);
                    }
                    Edit::Clipboard(ClipboardEdit::Cut) => {
                        query_editor::cut_selection(field, cx);
                    }
                    Edit::Clipboard(ClipboardEdit::Paste) => {
                        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                            field.insert(text.lines().next().unwrap_or_default());
                        }
                    }
                }
                let command = field.text().to_owned();
                self.update_prefs(move |prefs| prefs.talk_voice_command = command);
            }
        }
        cx.stop_propagation();
        cx.notify();
        true
    }

    /// A bare modifier is not a key-down. Left and right Option arrive as
    /// flags-changed with different hardware codes; record that on release
    /// when no other key completed the shortcut.
    pub(super) fn dictation_modifiers_changed(
        &mut self,
        event: &gpui::ModifiersChangedEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.surface != Surface::Settings
            || self.settings_tab != SettingsTab::Voice
            || !self.voice_fields.recording_shortcut
        {
            return false;
        }
        let (key_code, _) = physical_key();
        let outcome = self.voice_fields.gesture.on_flags(event.modifiers, key_code);
        self.apply_dictation_outcome(outcome, cx);
        cx.stop_propagation();
        true
    }

    fn arm_dictation_recording(&mut self, cx: &mut Context<Self>) {
        self.stop_dictation_recording();
        self.voice_fields.recording_shortcut = true;
        let entity = cx.entity();
        self.voice_fields.recording_guard = Some(cx.intercept_keystrokes(move |_event, _window, cx| {
            if entity.read(cx).voice_fields.recording_shortcut {
                cx.stop_propagation();
            }
        }));
    }

    fn stop_dictation_recording(&mut self) {
        self.voice_fields.recording_shortcut = false;
        self.voice_fields.recording_guard = None;
        self.voice_fields.gesture = DictationGesture::default();
    }

    fn apply_dictation_outcome(&mut self, outcome: GestureOutcome, cx: &mut Context<Self>) {
        match outcome {
            GestureOutcome::Listening => return,
            GestureOutcome::Cancel => self.stop_dictation_recording(),
            GestureOutcome::Clear => {
                self.stop_dictation_recording();
                self.update_prefs(|prefs| prefs.talk_answer_shortcut = None);
            }
            GestureOutcome::Record(shortcut) => {
                self.stop_dictation_recording();
                self.update_prefs(move |prefs| prefs.talk_answer_shortcut = Some(shortcut));
            }
        }
        cx.notify();
    }

    pub(super) fn voice_command_field(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.settings_colors();
        let active = self.voice_fields.command_active;
        let saved = self.prefs.talk_voice_command.clone();
        let content = if active {
            query_label(&self.voice_fields.command)
        } else if saved.is_empty() {
            div()
                .text_color(colors.tertiary)
                .child(t("settings.general.talk_voice_command_placeholder"))
                .into_any_element()
        } else {
            div().child(saved.clone()).into_any_element()
        };
        settings_text_box("talk-voice-command", 240.0, active, colors)
            .font_family(SharedString::from(crate::fonts::mono_family()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.deactivate_privacy_name();
                this.stop_dictation_recording();
                this.voice_fields.command = text_editor(&saved);
                this.voice_fields.command.select_all();
                this.voice_fields.command_active = true;
                this.roots_editor_active = false;
                this.include_editor_active = false;
                this.settings_search_active = false;
                this.focus.focus(window, cx);
                cx.notify();
            }))
            .child(content)
    }

    pub(super) fn dictation_shortcut_field(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.settings_colors();
        let recording = self.voice_fields.recording_shortcut;
        let label = if recording {
            t("settings.general.talk_shortcut_recording").to_owned()
        } else {
            self.prefs
                .talk_answer_shortcut
                .as_ref()
                .map(|shortcut| dictation_shortcut_label(&shortcut.label))
                .unwrap_or_else(|| t("settings.general.talk_shortcut_record").to_owned())
        };
        settings_text_box("talk-dictation-shortcut", 154.0, recording, colors)
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| {
                this.deactivate_privacy_name();
                this.voice_fields.command_active = false;
                this.arm_dictation_recording(cx);
                this.settings_search_active = false;
                this.focus.focus(window, cx);
                cx.notify();
            }))
            .child(label)
    }

    /// Which tabs are read: every tab, or only the ones turned on with a
    /// tab's speaker or ⌘K. Switching starts over from no exceptions.
    pub(super) fn talk_tabs_dropdown(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = self.settings_colors();
        let open = self.settings_menu == Some(SettingsMenu::TalkTabs);
        let mut control = div()
            .relative()
            .min_w(px(154.0))
            .child(settings_select_button(
                tabs_label(self.prefs.talk_reads_by_default),
                "talk-tabs-dropdown",
                open,
                SettingsMenu::TalkTabs,
                colors,
                cx,
            ));
        if open {
            control = control.child(self.settings_menu_host(cx));
        }
        control.into_any_element()
    }

    pub(super) fn talk_tabs_options(
        &self,
        colors: SemanticColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut options = div().p(px(4.0)).flex().flex_col();
        for (index, all) in [true, false].into_iter().enumerate() {
            options = options.child(settings_choice_row(
                format!("talk-tabs-option-{index}"),
                tabs_label(all),
                all == self.prefs.talk_reads_by_default,
                colors,
                cx,
                move |this, cx| {
                    this.settings_menu = None;
                    if all != this.prefs.talk_reads_by_default {
                        this.update_prefs(move |prefs| {
                            prefs.talk_reads_by_default = all;
                            prefs.talk_exceptions.clear();
                        });
                    }
                    cx.notify();
                },
            ));
        }
        options.into_any_element()
    }

    pub(super) fn talk_answer_dropdown(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = self.settings_colors();
        let open = self.settings_menu == Some(SettingsMenu::TalkAnswer);
        let mut control = div()
            .relative()
            .min_w(px(154.0))
            .child(settings_select_button(
                answer_label(self.prefs.talk_answer),
                "talk-answer-dropdown",
                open,
                SettingsMenu::TalkAnswer,
                colors,
                cx,
            ));
        if open {
            control = control.child(self.settings_menu_host(cx));
        }
        control.into_any_element()
    }

    pub(super) fn talk_answer_options(
        &self,
        colors: SemanticColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        #[cfg(target_os = "macos")]
        let wispr = crate::voice::wispr_flow_installed();
        #[cfg(not(target_os = "macos"))]
        let wispr = false;
        let mut options = div().p(px(4.0)).flex().flex_col();
        for (index, answer) in ANSWER_OPTIONS.into_iter().enumerate() {
            if answer == TalkAnswer::WisprFlow && !wispr {
                continue;
            }
            options = options.child(settings_choice_row(
                format!("talk-answer-option-{index}"),
                answer_label(answer),
                answer == self.prefs.talk_answer,
                colors,
                cx,
                move |this, cx| {
                    this.settings_menu = None;
                    this.update_prefs(move |prefs| prefs.talk_answer = answer);
                    cx.notify();
                },
            ));
        }
        options.into_any_element()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModifierKind {
    Control,
    Alt,
    Shift,
    Command,
    Function,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeySide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeldModifier {
    code: u16,
    kind: ModifierKind,
    side: Option<KeySide>,
}

impl HeldModifier {
    fn from_code(code: u16) -> Option<Self> {
        Some(match code {
            0x3B => Self { code, kind: ModifierKind::Control, side: Some(KeySide::Left) },
            0x3E => Self { code, kind: ModifierKind::Control, side: Some(KeySide::Right) },
            0x3A => Self { code, kind: ModifierKind::Alt, side: Some(KeySide::Left) },
            0x3D => Self { code, kind: ModifierKind::Alt, side: Some(KeySide::Right) },
            0x38 => Self { code, kind: ModifierKind::Shift, side: Some(KeySide::Left) },
            0x3C => Self { code, kind: ModifierKind::Shift, side: Some(KeySide::Right) },
            0x37 => Self { code, kind: ModifierKind::Command, side: Some(KeySide::Left) },
            0x36 => Self { code, kind: ModifierKind::Command, side: Some(KeySide::Right) },
            0x3F => Self { code, kind: ModifierKind::Function, side: None },
            _ => return None,
        })
    }

    fn is_down(self, modifiers: Modifiers) -> bool {
        match self.kind {
            ModifierKind::Control => modifiers.control,
            ModifierKind::Alt => modifiers.alt,
            ModifierKind::Shift => modifiers.shift,
            ModifierKind::Command => modifiers.platform,
            ModifierKind::Function => modifiers.function,
        }
    }
}

#[derive(Default)]
struct DictationGesture {
    held: Vec<HeldModifier>,
    saw_key: bool,
}

#[derive(Debug)]
enum GestureOutcome {
    Listening,
    Cancel,
    Clear,
    Record(DictationShortcut),
}

impl DictationGesture {
    fn on_flags(&mut self, modifiers: Modifiers, key_code: Option<u16>) -> GestureOutcome {
        if let Some(held) = key_code.and_then(HeldModifier::from_code)
            && held.is_down(modifiers)
        {
            self.held.retain(|item| item.code != held.code);
            self.held.push(held);
            return GestureOutcome::Listening;
        }
        if !modifiers.modified() && !self.saw_key && !self.held.is_empty() {
            return GestureOutcome::Record(shortcut_from_held(&self.held));
        }
        GestureOutcome::Listening
    }

    fn on_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        key_code: Option<u16>,
        flags: u64,
    ) -> GestureOutcome {
        if key == "escape" {
            return GestureOutcome::Cancel;
        }
        if matches!(key, "backspace" | "delete") && !modifiers.modified() {
            return GestureOutcome::Clear;
        }
        if is_modifier_name(key) || key_code.is_some_and(|code| HeldModifier::from_code(code).is_some())
        {
            return GestureOutcome::Listening;
        }
        let Some(key_code) = key_code else {
            return GestureOutcome::Listening;
        };
        self.saw_key = true;
        GestureOutcome::Record(DictationShortcut {
            label: label_for_key(key, modifiers, &self.held, flags),
            key_code,
        })
    }
}

fn shortcut_from_held(held: &[HeldModifier]) -> DictationShortcut {
    let key_code = held.last().map(|item| item.code).unwrap_or_default();
    let mut tokens = Vec::new();
    for kind in [
        ModifierKind::Function,
        ModifierKind::Control,
        ModifierKind::Alt,
        ModifierKind::Command,
        ModifierKind::Shift,
    ] {
        if let Some(item) = held.iter().rev().find(|item| item.kind == kind) {
            tokens.push(modifier_token(*item));
        }
    }
    DictationShortcut {
        label: tokens.join("-"),
        key_code,
    }
}

fn label_for_key(key: &str, modifiers: Modifiers, held: &[HeldModifier], flags: u64) -> String {
    let mut tokens = Vec::new();
    if modifiers.function {
        tokens.push("fn");
    }
    if modifiers.control {
        tokens.push(sided_token(ModifierKind::Control, held, flags));
    }
    if modifiers.alt {
        tokens.push(sided_token(ModifierKind::Alt, held, flags));
    }
    if modifiers.platform {
        tokens.push(sided_token(ModifierKind::Command, held, flags));
    }
    if modifiers.shift {
        tokens.push(sided_token(ModifierKind::Shift, held, flags));
    }
    tokens.push(key);
    tokens.join("-")
}

fn sided_token(kind: ModifierKind, held: &[HeldModifier], flags: u64) -> &'static str {
    let side = held
        .iter()
        .rev()
        .find(|item| item.kind == kind)
        .and_then(|item| item.side)
        .or_else(|| side_from_flags(kind, flags));
    modifier_token(HeldModifier {
        code: 0,
        kind,
        side,
    })
}

fn modifier_token(held: HeldModifier) -> &'static str {
    match (held.kind, held.side) {
        (ModifierKind::Control, Some(KeySide::Left)) => "leftctrl",
        (ModifierKind::Control, Some(KeySide::Right)) => "rightctrl",
        (ModifierKind::Control, None) => "ctrl",
        (ModifierKind::Alt, Some(KeySide::Left)) => "leftalt",
        (ModifierKind::Alt, Some(KeySide::Right)) => "rightalt",
        (ModifierKind::Alt, None) => "alt",
        (ModifierKind::Shift, Some(KeySide::Left)) => "leftshift",
        (ModifierKind::Shift, Some(KeySide::Right)) => "rightshift",
        (ModifierKind::Shift, None) => "shift",
        (ModifierKind::Command, Some(KeySide::Left)) => "leftcmd",
        (ModifierKind::Command, Some(KeySide::Right)) => "rightcmd",
        (ModifierKind::Command, None) => "cmd",
        (ModifierKind::Function, _) => "fn",
    }
}

fn side_from_flags(kind: ModifierKind, flags: u64) -> Option<KeySide> {
    let (left, right) = match kind {
        ModifierKind::Control => (0x1, 0x2000),
        ModifierKind::Alt => (0x20, 0x40),
        ModifierKind::Shift => (0x2, 0x4),
        ModifierKind::Command => (0x8, 0x10),
        ModifierKind::Function => return None,
    };
    match (flags & left != 0, flags & right != 0) {
        (true, false) => Some(KeySide::Left),
        (false, true) => Some(KeySide::Right),
        _ => None,
    }
}

fn is_modifier_name(key: &str) -> bool {
    matches!(
        key,
        "shift" | "control" | "ctrl" | "alt" | "platform" | "function" | "fn"
    )
}

fn physical_key() -> (Option<u16>, u64) {
    #[cfg(target_os = "macos")]
    {
        gpui_macos::current_physical_key()
            .map(|(code, flags)| (Some(code), flags))
            .unwrap_or((None, 0))
    }
    #[cfg(not(target_os = "macos"))]
    {
        (None, 0)
    }
}

/// How the recorded shortcut reads in the field. Sided modifiers stay
/// distinct; an older unsided label keeps the normal shortcut spelling.
fn dictation_shortcut_label(label: &str) -> String {
    let parts: Vec<&str> = label.split('-').collect();
    if !parts.iter().any(|part| sided_symbol(part).is_some()) {
        return crate::commands::keystroke_label(label);
    }
    let mut out = String::new();
    for (index, part) in parts.iter().enumerate() {
        if let Some(symbol) = sided_symbol(part) {
            out.push_str(symbol);
        } else if let Some(symbol) = generic_symbol(part) {
            out.push_str(symbol);
        } else if index + 1 == parts.len() {
            out.push_str(&key_symbol(part));
        }
    }
    out
}

fn sided_symbol(part: &str) -> Option<&'static str> {
    Some(match part {
        "leftalt" => "Left ⌥",
        "rightalt" => "Right ⌥",
        "leftshift" => "Left ⇧",
        "rightshift" => "Right ⇧",
        "leftctrl" => "Left ⌃",
        "rightctrl" => "Right ⌃",
        "leftcmd" => "Left ⌘",
        "rightcmd" => "Right ⌘",
        _ => return None,
    })
}

fn generic_symbol(part: &str) -> Option<&'static str> {
    Some(match part {
        "fn" => "fn",
        "ctrl" => "⌃",
        "alt" => "⌥",
        "shift" => "⇧",
        "cmd" => "⌘",
        _ => return None,
    })
}

fn key_symbol(key: &str) -> String {
    match key {
        "backspace" => "⌫".into(),
        "delete" => "⌦".into(),
        "enter" => "↩".into(),
        "tab" => "⇥".into(),
        "escape" => "⎋".into(),
        "space" => "Space".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// The bordered box Settings text fields share.
fn settings_text_box(
    id: &'static str,
    width: f32,
    active: bool,
    colors: SemanticColors,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .w(px(width))
        .h(px(26.0))
        .px(px(8.0))
        .rounded(px(Radius::BADGE))
        .border_1()
        .border_color(colors.primary.alpha(if active { 0.26 } else { 0.10 }))
        .bg(colors.primary.alpha(if active { 0.075 } else { 0.04 }))
        .flex()
        .items_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(11.0))
        .text_color(colors.primary)
        .cursor(CursorStyle::IBeam)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(alt: bool, command: bool) -> Modifiers {
        Modifiers {
            alt,
            platform: command,
            ..Modifiers::default()
        }
    }

    fn recorded(outcome: GestureOutcome) -> DictationShortcut {
        match outcome {
            GestureOutcome::Record(shortcut) => shortcut,
            other => panic!("expected a recorded shortcut, got {other:?}"),
        }
    }

    #[test]
    fn releasing_right_option_records_that_key() {
        let mut gesture = DictationGesture::default();
        assert!(matches!(
            gesture.on_flags(mods(true, false), Some(0x3D)),
            GestureOutcome::Listening
        ));
        let shortcut = recorded(gesture.on_flags(Modifiers::default(), Some(0x3D)));
        assert_eq!(shortcut.key_code, 0x3D);
        assert_eq!(shortcut.label, "rightalt");
        assert_eq!(shortcut.modifiers(), vec!["alt-right"]);
        assert_eq!(dictation_shortcut_label(&shortcut.label), "Right ⌥");
    }

    #[test]
    fn left_and_right_option_stay_distinct() {
        let mut left = DictationGesture::default();
        left.on_flags(mods(true, false), Some(0x3A));
        let left = recorded(left.on_flags(Modifiers::default(), Some(0x3A)));
        let mut right = DictationGesture::default();
        right.on_flags(mods(true, false), Some(0x3D));
        let right = recorded(right.on_flags(Modifiers::default(), Some(0x3D)));
        assert_eq!(left.key_code, 0x3A);
        assert_eq!(left.modifiers(), vec!["alt-left"]);
        assert_eq!(dictation_shortcut_label(&left.label), "Left ⌥");
        assert_ne!(left, right);
    }

    #[test]
    fn a_chord_keeps_the_option_side_from_the_held_key() {
        let mut gesture = DictationGesture::default();
        gesture.on_flags(mods(true, false), Some(0x3D));
        let shortcut = recorded(gesture.on_key("d", mods(true, false), Some(2), 0));
        assert_eq!(shortcut.key_code, 2);
        assert_eq!(shortcut.label, "rightalt-d");
        assert_eq!(shortcut.modifiers(), vec!["alt-right"]);
        assert_eq!(dictation_shortcut_label(&shortcut.label), "Right ⌥D");
    }

    #[test]
    fn device_flags_distinguish_option_when_the_hold_was_missed() {
        let mut gesture = DictationGesture::default();
        let shortcut = recorded(gesture.on_key("d", mods(true, false), Some(2), 0x20));
        assert_eq!(shortcut.label, "leftalt-d");
        let mut other = DictationGesture::default();
        let other = recorded(other.on_key("d", mods(true, false), Some(2), 0x40));
        assert_eq!(other.label, "rightalt-d");
    }

    #[test]
    fn two_modifiers_record_when_the_last_one_is_released() {
        let mut gesture = DictationGesture::default();
        gesture.on_flags(mods(false, true), Some(0x37));
        gesture.on_flags(mods(true, true), Some(0x3D));
        assert!(matches!(
            gesture.on_flags(mods(true, false), Some(0x37)),
            GestureOutcome::Listening
        ));
        let shortcut = recorded(gesture.on_flags(Modifiers::default(), Some(0x3D)));
        assert_eq!(shortcut.key_code, 0x3D);
        assert_eq!(shortcut.label, "rightalt-leftcmd");
        assert_eq!(shortcut.modifiers(), vec!["alt-right", "cmd-left"]);
    }

    #[test]
    fn escape_cancels_and_a_missing_key_code_keeps_listening() {
        let mut gesture = DictationGesture::default();
        assert!(matches!(
            gesture.on_key("escape", Modifiers::default(), Some(53), 0),
            GestureOutcome::Cancel
        ));
        let mut gesture = DictationGesture::default();
        assert!(matches!(
            gesture.on_key("d", mods(true, false), None, 0x40),
            GestureOutcome::Listening
        ));
    }

    #[test]
    fn an_unsided_shortcut_keeps_the_normal_label() {
        assert_eq!(
            dictation_shortcut_label("cmd-alt-d"),
            crate::commands::keystroke_label("cmd-alt-d")
        );
    }
}
