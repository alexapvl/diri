//! Digit-reel number animation, matching barvian/number-flow.
#[cfg(test)]
use crate::usage::UsageFormat;
use gpui::{
    AnyElement, App, Bounds, ContentMask, Element, ElementId, FontFeatures, FontWeight,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Rgba, ShapedLine,
    SharedString, Style, TextAlign, TextStyle, Window, point, px, size,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DIGIT_EM: f32 = 0.62;
const SPIN: Duration = Duration::from_millis(900);

#[derive(Default)]
pub struct Bank {
    slots: RefCell<HashMap<String, Slot>>,
}

struct Slot {
    from: f64,
    to: f64,
    from_text: String,
    to_text: String,
    start: Option<Instant>,
}

impl Bank {
    pub fn show(
        &self,
        id: impl Into<String>,
        value: f64,
        text: impl Into<String>,
        size: f32,
        color: Rgba,
        weight: FontWeight,
    ) -> AnyElement {
        self.show_font(id, value, text, size, color, weight, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show_font(
        &self,
        id: impl Into<String>,
        value: f64,
        text: impl Into<String>,
        size: f32,
        color: Rgba,
        weight: FontWeight,
        font: Option<SharedString>,
    ) -> AnyElement {
        let text = text.into();
        let mut slots = self.slots.borrow_mut();
        let slot = slots.entry(id.into()).or_insert_with(|| Slot {
            from: value,
            to: value,
            from_text: text.clone(),
            to_text: text.clone(),
            start: None,
        });
        if format_kind(&slot.to_text) != format_kind(&text) {
            slot.from = value;
            slot.to = value;
            slot.from_text = text.clone();
            slot.to_text = text.clone();
            slot.start = None;
        } else if (value - slot.to).abs() > 0.000_5 {
            let t = progress(slot);
            let was = trend(slot.from, slot.to);
            slot.from_text = snapshot(&slot.from_text, &slot.to_text, was, t);
            slot.from = slot.to;
            slot.to = value;
            slot.to_text = text.clone();
            slot.start = Some(Instant::now());
        } else {
            slot.to_text = text.clone();
            if slot.start.is_none() {
                slot.from_text = text.clone();
            }
        }
        let t = progress(slot);
        let dir = trend(slot.from, slot.to);
        let from_text = slot.from_text.clone();
        let to_text = slot.to_text.clone();
        if done(slot) {
            slot.from = slot.to;
            slot.from_text = to_text.clone();
            slot.start = None;
        }
        drop(slots);
        NumberText::new(flow_cells(&from_text, &to_text, t, dir), size, color, weight)
            .font(font)
            .into_any_element()
    }

    pub fn running(&self) -> bool {
        self.slots.borrow().values().any(|slot| !done(slot))
    }
}

fn progress(slot: &Slot) -> f32 {
    let Some(start) = slot.start else {
        return 1.0;
    };
    ease((start.elapsed().as_secs_f32() / SPIN.as_secs_f32()).clamp(0.0, 1.0))
}

fn done(slot: &Slot) -> bool {
    slot.start
        .is_none_or(|start| start.elapsed() >= SPIN || (slot.from - slot.to).abs() <= 0.000_5)
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}

fn trend(from: f64, to: f64) -> i8 {
    if to > from {
        1
    } else if to < from {
        -1
    } else {
        0
    }
}

fn digit_delta(from: u8, to: u8, dir: i8) -> i32 {
    let from = i32::from(from);
    let to = i32::from(to);
    if dir < 0 && to > from {
        to - 10 - from
    } else if dir > 0 && to < from {
        10 - from + to
    } else {
        to - from
    }
}

fn glyph(n: f32) -> char {
    char::from(b'0' + (n as i32).rem_euclid(10) as u8)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FormatKind {
    Money,
    Tokens,
    Percent,
    Int,
}

fn format_kind(sample: &str) -> FormatKind {
    if sample.starts_with('$') {
        FormatKind::Money
    } else if sample.ends_with('%') {
        FormatKind::Percent
    } else if sample.chars().any(|ch| ch.is_ascii_alphabetic()) {
        FormatKind::Tokens
    } else {
        FormatKind::Int
    }
}

struct Parts<'a> {
    prefix: &'a str,
    integer: &'a str,
    fraction: &'a str,
    suffix: &'a str,
    dot: bool,
}

fn split_format(s: &str) -> Parts<'_> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_digit() {
        i += 1;
    }
    let mut j = i;
    while j < bytes.len() && bytes[j].is_ascii_digit() {
        j += 1;
    }
    let (dot, frac_at) = if j < bytes.len() && bytes[j] == b'.' {
        (true, j + 1)
    } else {
        (false, j)
    };
    let mut k = frac_at;
    while k < bytes.len() && bytes[k].is_ascii_digit() {
        k += 1;
    }
    Parts {
        prefix: &s[..i],
        integer: &s[i..j],
        fraction: &s[frac_at..k],
        suffix: &s[k..],
        dot,
    }
}

fn int_digit(digits: &str, width: usize, i: usize) -> Option<u8> {
    let skip = width.saturating_sub(digits.len());
    if i < skip {
        None
    } else {
        digits.as_bytes().get(i - skip).map(|b| b - b'0')
    }
}

fn frac_digit(digits: &str, _width: usize, i: usize) -> Option<u8> {
    (i < digits.len()).then(|| digits.as_bytes()[i] - b'0')
}

fn snapshot_digit(from: Option<u8>, to: Option<u8>, dir: i8, t: f32) -> Option<char> {
    match (from, to) {
        (None, None) => None,
        (Some(a), Some(b)) => Some(glyph(f32::from(a) + digit_delta(a, b, dir) as f32 * t)),
        (None, Some(b)) => {
            if t <= 0.0 {
                None
            } else {
                Some(glyph(digit_delta(0, b, dir) as f32 * t))
            }
        }
        (Some(a), None) => {
            if t >= 1.0 {
                None
            } else {
                Some(glyph(f32::from(a) + digit_delta(a, 0, dir) as f32 * t))
            }
        }
    }
}

fn snapshot(from: &str, to: &str, dir: i8, t: f32) -> String {
    if t <= 0.0 {
        return from.to_owned();
    }
    if t >= 1.0 {
        return to.to_owned();
    }
    let a = split_format(from);
    let b = split_format(to);
    let mut out = String::new();
    out.push_str(if t < 0.5 { a.prefix } else { b.prefix });
    let int_w = a.integer.len().max(b.integer.len());
    for i in 0..int_w {
        if let Some(ch) = snapshot_digit(
            int_digit(a.integer, int_w, i),
            int_digit(b.integer, int_w, i),
            dir,
            t,
        ) {
            out.push(ch);
        }
    }
    if a.dot || b.dot {
        out.push('.');
    }
    let frac_w = a.fraction.len().max(b.fraction.len());
    for i in 0..frac_w {
        if let Some(ch) = snapshot_digit(
            frac_digit(a.fraction, frac_w, i),
            frac_digit(b.fraction, frac_w, i),
            dir,
            t,
        ) {
            out.push(ch);
        }
    }
    out.push_str(if t < 0.5 { a.suffix } else { b.suffix });
    out
}

/// One piece of a number: a digit in its own fixed-width cell, or other
/// text at its natural width. A digit's `value` is its reel position, so
/// `4.25` shows 4 rolling a quarter of the way toward 5.
#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Digit { value: f32, scale: f32, alpha: f32 },
    Text { text: SharedString, alpha: f32 },
}

/// A number drawn as one element. Every digit keeps its own `DIGIT_EM`
/// cell, so the width only changes when a digit is added or removed; a
/// rolling digit is two glyphs clipped to its cell. One layout node of
/// definite size replaces a flex row holding a box per character, whose
/// layout dominated every frame of a page full of numbers.
pub(crate) struct NumberText {
    cells: Vec<Cell>,
    size: f32,
    color: Rgba,
    weight: FontWeight,
    font: Option<SharedString>,
}

pub(crate) struct NumberTextLayout {
    style: TextStyle,
    line_height: Pixels,
    /// The `Text` cells, shaped at full opacity, in order.
    texts: Vec<ShapedLine>,
}

impl NumberText {
    fn new(cells: Vec<Cell>, size: f32, color: Rgba, weight: FontWeight) -> Self {
        Self {
            cells,
            size,
            color,
            weight,
            font: None,
        }
    }

    pub(crate) fn font(mut self, font: Option<SharedString>) -> Self {
        self.font = font;
        self
    }
}

fn shape(window: &mut Window, text: SharedString, style: &TextStyle, size: f32) -> ShapedLine {
    let run = style.to_run(text.len());
    window
        .text_system()
        .shape_line(text, px(size), &[run], None)
}

impl IntoElement for NumberText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for NumberText {
    type RequestLayoutState = NumberTextLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = window.text_style();
        style.font_size = px(self.size).into();
        style.font_weight = self.weight;
        style.font_features = FontFeatures(Arc::new(vec![("tnum".into(), 1)]));
        style.color = self.color.into();
        if let Some(family) = &self.font {
            style.font_family = family.clone();
        }
        let line_height = window.pixel_snap(px(self.size));
        let digit_w = self.size * DIGIT_EM;
        let mut width = 0.0;
        let mut texts = Vec::new();
        for cell in &self.cells {
            match cell {
                Cell::Digit { scale, .. } => width += digit_w * scale,
                Cell::Text { text, .. } => {
                    let line = shape(window, text.clone(), &style, self.size);
                    // A text element measures to its shaped width, rounded up.
                    width += f32::from(line.width.ceil());
                    texts.push(line);
                }
            }
        }
        let layout = Style {
            size: size(px(width).into(), line_height.into()),
            flex_shrink: 0.0,
            ..Style::default()
        };
        (
            window.request_layout(layout, None, cx),
            NumberTextLayout {
                style,
                line_height,
                texts,
            },
        )
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let digit_w = self.size * DIGIT_EM;
        let line_height = layout.line_height;
        let top = bounds.origin.y;
        let mut x = f32::from(bounds.origin.x);
        let mut texts = layout.texts.iter();
        let faded = |alpha: f32| {
            let mut style = layout.style.clone();
            style.color = Rgba {
                a: self.color.a * alpha.clamp(0.0, 1.0),
                ..self.color
            }
            .into();
            style
        };
        for cell in &self.cells {
            match cell {
                Cell::Text { text, alpha } => {
                    let Some(line) = texts.next() else {
                        continue;
                    };
                    let origin = point(px(x), top);
                    if *alpha >= 1.0 {
                        let _ = line.paint(origin, line_height, TextAlign::Left, None, window, cx);
                    } else if *alpha > 0.0 {
                        let line = shape(window, text.clone(), &faded(*alpha), self.size);
                        let _ = line.paint(origin, line_height, TextAlign::Left, None, window, cx);
                    }
                    x += f32::from(line.width.ceil());
                }
                Cell::Digit {
                    value,
                    scale,
                    alpha,
                } => {
                    let cell_w = digit_w * scale;
                    if *scale > 0.0 && *alpha > 0.0 {
                        let style = faded(*alpha);
                        let below = value.floor();
                        let frac = value - below;
                        // Each glyph is centred in the cell, as the flex cell
                        // centred it, and offset vertically while it rolls.
                        let mut glyph_at = |n: f32, dy: f32, window: &mut Window| {
                            let line = shape(
                                window,
                                SharedString::from(glyph(n).to_string()),
                                &style,
                                self.size,
                            );
                            let gx = x + (cell_w - f32::from(line.width)) * 0.5;
                            let _ = line.paint(
                                point(px(gx), top + px(dy)),
                                line_height,
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        };
                        if frac <= 0.0 && *scale >= 1.0 {
                            glyph_at(below, 0.0, window);
                        } else {
                            let cell = Bounds::new(point(px(x), top), size(px(cell_w), line_height));
                            window.with_content_mask(Some(ContentMask { bounds: cell }), |window| {
                                glyph_at(below, -frac * self.size, window);
                                if frac > 0.0 {
                                    glyph_at(below + 1.0, (1.0 - frac) * self.size, window);
                                }
                            });
                        }
                    }
                    x += cell_w;
                }
            }
        }
    }
}

/// The cells of a number rolling from `from` to `to`, `t` of the way.
fn flow_cells(from: &str, to: &str, t: f32, dir: i8) -> Vec<Cell> {
    if t >= 1.0 || from == to {
        return tabular_cells(to);
    }
    let a = split_format(from);
    let b = split_format(to);
    let mut cells = Vec::new();
    push_symbol(&mut cells, a.prefix, b.prefix, t);
    let int_w = a.integer.len().max(b.integer.len());
    for i in 0..int_w {
        push_digit(
            &mut cells,
            int_digit(a.integer, int_w, i),
            int_digit(b.integer, int_w, i),
            dir,
            t,
        );
    }
    if a.dot || b.dot {
        cells.push(Cell::Text {
            text: ".".into(),
            alpha: 1.0,
        });
    }
    let frac_w = a.fraction.len().max(b.fraction.len());
    for i in 0..frac_w {
        push_digit(
            &mut cells,
            frac_digit(a.fraction, frac_w, i),
            frac_digit(b.fraction, frac_w, i),
            dir,
            t,
        );
    }
    push_symbol(&mut cells, a.suffix, b.suffix, t);
    cells
}

fn push_symbol(cells: &mut Vec<Cell>, from: &str, to: &str, t: f32) {
    let (text, alpha) = if from == to {
        (from, 1.0)
    } else if to.is_empty() {
        (from, 1.0 - t)
    } else {
        (to, t)
    };
    if !text.is_empty() {
        cells.push(Cell::Text {
            text: SharedString::from(text.to_owned()),
            alpha: alpha.clamp(0.0, 1.0),
        });
    }
}

fn cell_scale(from: Option<u8>, to: Option<u8>, t: f32) -> f32 {
    match (from, to) {
        (None, None) => 0.0,
        (Some(_), Some(_)) => 1.0,
        (None, Some(_)) => t.clamp(0.0, 1.0),
        (Some(_), None) => (1.0 - t).clamp(0.0, 1.0),
    }
}

fn push_digit(cells: &mut Vec<Cell>, from: Option<u8>, to: Option<u8>, dir: i8, t: f32) {
    let scale = cell_scale(from, to, t);
    if scale <= 0.0 {
        return;
    }
    let (a, b, alpha) = match (from, to) {
        (None, None) => return,
        (Some(a), Some(b)) => (a, b, 1.0),
        (None, Some(b)) => (0, b, t),
        (Some(a), None) => (a, 0, 1.0 - t),
    };
    cells.push(Cell::Digit {
        value: f32::from(a) + digit_delta(a, b, dir) as f32 * t,
        scale,
        alpha: alpha.clamp(0.0, 1.0),
    });
}

/// Digits in fixed cells, everything else one character at a time at its
/// own width.
fn tabular_cells(text: &str) -> Vec<Cell> {
    text.chars()
        .map(|ch| match ch.to_digit(10) {
            Some(digit) => Cell::Digit {
                value: digit as f32,
                scale: 1.0,
                alpha: 1.0,
            },
            None => Cell::Text {
                text: SharedString::from(ch.to_string()),
                alpha: 1.0,
            },
        })
        .collect()
}

pub(crate) fn tabular(
    text: impl AsRef<str>,
    size: f32,
    color: Rgba,
    weight: FontWeight,
) -> NumberText {
    NumberText::new(tabular_cells(text.as_ref()), size, color, weight)
}

pub(crate) fn tabular_width(text: &str, size: f32) -> f32 {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_digit() {
                size * DIGIT_EM
            } else if ch == ' ' {
                size * 0.28
            } else {
                size * 0.5
            }
        })
        .sum()
}

#[cfg(test)]
fn digit_count(text: &str) -> usize {
    text.chars().filter(|ch| ch.is_ascii_digit()).count()
}

#[cfg(test)]
fn format_like(sample: &str, value: f64) -> String {
    match format_kind(sample) {
        FormatKind::Money => format!("${value:.2}"),
        FormatKind::Percent => format!("{}%", value.round() as i64),
        FormatKind::Tokens => UsageFormat::tokens(value.round() as i64),
        FormatKind::Int => (value.round() as i64).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_like_keeps_money_and_compact_tokens() {
        assert_eq!(format_like("$41.65", 1000.0), "$1000.00");
        assert_eq!(format_like("$0.00", 41.65), "$41.65");
        assert_eq!(format_like("41.7M", 1_000_000_000.0), "1.0B");
        assert_eq!(format_like("81%", 12.4), "12%");
        assert_eq!(format_like("20", 19.6), "20");
    }

    #[test]
    fn reel_follows_number_flow_trend() {
        assert_eq!(digit_delta(4, 5, 1), 1);
        assert_eq!(digit_delta(5, 4, -1), -1);
        assert_eq!(digit_delta(9, 0, 1), 1);
        assert_eq!(digit_delta(0, 9, -1), -1);
        assert_eq!(digit_delta(1, 8, 1), 7);
        assert_eq!(digit_delta(8, 1, -1), -7);
    }

    #[test]
    fn snapshot_keeps_place_values() {
        assert_eq!(snapshot("$41.65", "$42.10", 1, 0.0), "$41.65");
        assert_eq!(snapshot("$41.65", "$42.10", 1, 1.0), "$42.10");
        let mid = snapshot("$9.00", "$10.00", 1, 0.5);
        assert!(mid.starts_with('$'), "{mid}");
        assert!(mid.contains('.'), "{mid}");
    }

    #[test]
    fn leaving_digit_width_eases_out() {
        assert!((cell_scale(Some(1), None, 0.0) - 1.0).abs() < 0.001);
        assert!((cell_scale(Some(1), None, 0.5) - 0.5).abs() < 0.001);
        assert!(cell_scale(Some(1), None, 1.0) < 0.001);
        assert!((cell_scale(None, Some(1), 0.5) - 0.5).abs() < 0.001);
        assert!((cell_scale(Some(4), Some(2), 0.3) - 1.0).abs() < 0.001);
    }

    fn digit_space(cells: &[Cell]) -> f32 {
        cells
            .iter()
            .map(|cell| match cell {
                Cell::Digit { scale, .. } => *scale,
                Cell::Text { .. } => 0.0,
            })
            .sum()
    }

    #[test]
    fn every_digit_keeps_its_own_cell() {
        let cells = tabular_cells("$44.50");
        assert_eq!(
            cells,
            vec![
                Cell::Text { text: "$".into(), alpha: 1.0 },
                Cell::Digit { value: 4.0, scale: 1.0, alpha: 1.0 },
                Cell::Digit { value: 4.0, scale: 1.0, alpha: 1.0 },
                Cell::Text { text: ".".into(), alpha: 1.0 },
                Cell::Digit { value: 5.0, scale: 1.0, alpha: 1.0 },
                Cell::Digit { value: 0.0, scale: 1.0, alpha: 1.0 },
            ]
        );
        // Mid-roll, the same four cells hold the reels: nothing shifts.
        for t in [0.0, 0.3, 0.5, 0.9] {
            let rolling = flow_cells("$44.10", "$45.10", t, 1);
            assert_eq!(digit_space(&rolling), 4.0, "t={t}");
            assert_eq!(rolling.len(), cells.len(), "t={t}");
        }
        let Cell::Digit { value, .. } = flow_cells("$44.10", "$45.10", 0.5, 1)[2] else {
            panic!("second integer digit is a reel");
        };
        assert!((value - 4.5).abs() < 0.001, "{value}");
        // A new digit's cell opens as it rolls in, then holds a full cell.
        assert_eq!(digit_space(&flow_cells("$999.00", "$1000.00", 0.5, 1)), 5.5);
        assert_eq!(digit_space(&flow_cells("$999.00", "$1000.00", 1.0, 1)), 6.0);
    }

    #[test]
    fn width_holds_until_a_new_digit() {
        assert_eq!(digit_count("$44.10"), digit_count("$45.10"));
        assert_eq!(tabular_width("$44.10", 12.0), tabular_width("$45.10", 12.0));
        assert!(digit_count("$1000.00") > digit_count("$999.00"));
        assert!(tabular_width("$1000.00", 12.0) > tabular_width("$999.00", 12.0));
    }
}
