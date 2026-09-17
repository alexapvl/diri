//! On-device usage share card: caption, tweet intent, and a compact PNG.
use crate::usage::UsageFormat;
use crate::usage::dashboard::{ModelRow, UsageReport};
use gpui::Rgba;
use image::RgbaImage;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

const PROVIDERS: [&str; 3] = ["Claude", "Codex", "Cursor"];
const BRAND: &str = "diri.sh";
pub(super) const CARD_W: f32 = 600.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HostLabel {
    Hidden,
    All,
    ThisMac,
    Named(String),
}

impl HostLabel {
    fn phrase(&self) -> Option<&str> {
        match self {
            Self::Hidden => None,
            Self::All => Some("all machines"),
            Self::ThisMac => Some("this Mac"),
            Self::Named(name) => Some(name.as_str()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShareOptions {
    pub tokens: bool,
    pub individual: bool,
    pub include_models: bool,
    pub theme_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShareProvider {
    pub index: usize,
    pub name: &'static str,
    pub value: String,
    pub detail: String,
    pub share: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShareModel {
    pub name: String,
    pub provider: &'static str,
    pub value: String,
    pub share: String,
    pub tokens: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShareSeries {
    pub provider: Option<usize>,
    pub points: Vec<(f32, f32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ShareCard {
    pub days: usize,
    pub tokens: bool,
    pub host: HostLabel,
    pub hero: String,
    pub providers: Vec<ShareProvider>,
    pub graph: Vec<ShareSeries>,
    pub models: Option<Vec<ShareModel>>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SharePalette {
    pub background: Rgba,
    pub primary: Rgba,
    pub secondary: Rgba,
    pub tertiary: Rgba,
    pub providers: [Rgba; 3],
}

#[derive(Clone)]
pub(super) struct SharePreview {
    pub image: Option<Arc<gpui::Image>>,
    pub caption: String,
    pub file_name: String,
    pub options: ShareOptions,
    pub height: f32,
    pub theme_menu: bool,
    pub display_theme: String,
    pub pending_theme: Option<String>,
    pub cache: HashMap<(String, bool, bool, bool), Arc<gpui::Image>>,
}

impl SharePreview {
    pub(super) fn cache_key(theme_id: &str, options: &ShareOptions) -> (String, bool, bool, bool) {
        (
            theme_id.to_owned(),
            options.include_models,
            options.tokens,
            options.individual,
        )
    }

    pub(super) fn from_card(
        card: &ShareCard,
        palette: SharePalette,
        options: ShareOptions,
        mut cache: HashMap<(String, bool, bool, bool), Arc<gpui::Image>>,
        theme_menu: bool,
    ) -> Self {
        let key = Self::cache_key(&options.theme_id, &options);
        let image = cache.get(&key).cloned().or_else(|| {
            render_png(card, palette).map(|png| {
                let image = Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png));
                cache.insert(key, Arc::clone(&image));
                image
            })
        });
        Self {
            image,
            caption: card.caption(),
            file_name: card.file_name(),
            display_theme: options.theme_id.clone(),
            pending_theme: None,
            theme_menu,
            cache,
            options,
            height: card.height(),
        }
    }
}

impl ShareCard {
    pub(super) fn from_report(
        report: &UsageReport,
        days: usize,
        host: HostLabel,
        options: &ShareOptions,
        graph: Vec<ShareSeries>,
    ) -> Self {
        let total = report.total.totals();
        let tokens = options.tokens;
        let hero = if tokens {
            UsageFormat::tokens(total.total_tokens())
        } else {
            money(total.cost)
        };
        let providers = (0..3)
            .filter(|&index| provider_visible(&report.providers[index], tokens))
            .map(|index| {
                let provider = report.providers[index];
                let provider_tokens = provider.totals().total_tokens();
                let share = if tokens {
                    ratio(provider_tokens as f64, total.total_tokens() as f64)
                } else {
                    ratio(provider.tokens.c, total.cost)
                };
                ShareProvider {
                    index,
                    name: PROVIDERS[index],
                    value: if tokens {
                        UsageFormat::tokens(provider_tokens)
                    } else {
                        money(provider.tokens.c)
                    },
                    detail: if tokens {
                        format!(
                            "{:.1}% of tokens · {}",
                            share * 100.0,
                            money(provider.tokens.c)
                        )
                    } else {
                        format!(
                            "{:.1}% of cost · {} tokens",
                            share * 100.0,
                            UsageFormat::tokens(provider_tokens)
                        )
                    },
                    share,
                }
            })
            .collect();
        let models = options.include_models.then(|| top_models(report, tokens));
        Self {
            days,
            tokens,
            host,
            hero,
            providers,
            graph,
            models,
        }
    }

    pub(super) fn period(&self) -> &'static str {
        period_phrase(self.days)
    }

    pub(super) fn height(&self) -> f32 {
        let mut height = 214.0;
        height += 46.0 * self.providers.len() as f32;
        if let Some(models) = &self.models {
            height += 26.0 + 24.0 * models.len() as f32;
        }
        (height + 20.0).max(300.0)
    }

    pub(super) fn caption(&self) -> String {
        let mut head = format!("{BRAND} · {}", self.period());
        if let Some(host) = self.host.phrase() {
            head = format!("{BRAND} · {host} · {}", self.period());
        }
        let hero = if self.tokens {
            format!("{} tokens", self.hero)
        } else {
            self.hero.clone()
        };
        let split = self
            .providers
            .iter()
            .map(|provider| format!("{} {}", provider.name, provider.value))
            .collect::<Vec<_>>()
            .join(" · ");
        let mut caption = format!("{head} · {hero}\n{split}");
        if let Some(models) = &self.models
            && !models.is_empty()
        {
            let models = models
                .iter()
                .map(|model| format!("{} {}", model.name, model.value))
                .collect::<Vec<_>>()
                .join(" · ");
            caption = format!("{head} · {hero}\n{split}\n{models}");
        }
        caption
    }

    pub(super) fn file_name(&self) -> String {
        let range = match self.days {
            1 => "24h",
            7 => "7d",
            30 => "30d",
            90 => "90d",
            days => return format!("diri-usage-{days}d.png"),
        };
        let metric = if self.tokens { "tokens" } else { "cost" };
        format!("diri-usage-{range}-{metric}.png")
    }
}

pub(super) fn tweet_intent_url(caption: &str) -> String {
    let mut url = url::Url::parse("https://x.com/intent/tweet").expect("static tweet intent");
    url.query_pairs_mut().append_pair("text", caption);
    url.to_string()
}

pub(super) fn save_directory() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| {
            let downloads = home.join("Downloads");
            if downloads.is_dir() { downloads } else { home }
        })
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub(super) fn encode_png(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut png, image::ImageFormat::Png)
        .ok()?;
    Some(png.into_inner())
}

pub(super) fn render_png(card: &ShareCard, palette: SharePalette) -> Option<Vec<u8>> {
    encode_png(&rasterize(card, palette)?)
}

fn period_phrase(days: usize) -> &'static str {
    match days {
        1 => "last 24 hours",
        7 => "last 7 days",
        30 => "last 30 days",
        90 => "last 90 days",
        _ => "this period",
    }
}

fn money(value: f64) -> String {
    format!("${value:.2}")
}

fn ratio(value: f64, total: f64) -> f64 {
    if total > 0.0 {
        (value / total).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn provider_visible(provider: &crate::usage::dashboard::UsageDetail, tokens: bool) -> bool {
    if tokens {
        provider.totals().total_tokens() > 0
    } else {
        provider.tokens.c > 0.0
    }
}

fn top_models(report: &UsageReport, tokens: bool) -> Vec<ShareModel> {
    let mut rows: Vec<&ModelRow> = report
        .models
        .iter()
        .filter(|row| provider_visible(&row.detail, tokens))
        .collect();
    rows.sort_by(|left, right| {
        let key = |row: &ModelRow| {
            if tokens {
                row.detail.totals().total_tokens() as f64
            } else {
                row.detail.totals().cost
            }
        };
        key(right)
            .partial_cmp(&key(left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let total = if tokens {
        report.total.totals().total_tokens() as f64
    } else {
        report.total.tokens.c
    };
    rows.into_iter()
        .take(3)
        .map(|row| {
            let detail = row.detail.totals();
            let share = if tokens {
                ratio(detail.total_tokens() as f64, total)
            } else {
                ratio(detail.cost, total)
            };
            ShareModel {
                name: short_model(&row.model),
                provider: PROVIDERS[row.provider.min(2)],
                value: if tokens {
                    UsageFormat::tokens(detail.total_tokens())
                } else if row.detail.priced_tokens == 0 {
                    "Unpriced".into()
                } else {
                    money(detail.cost)
                },
                share: format!("{:.0}%", share * 100.0),
                tokens: UsageFormat::tokens(detail.total_tokens()),
            }
        })
        .collect()
}

fn short_model(name: &str) -> String {
    name.rsplit(['/', ':']).next().unwrap_or(name).to_owned()
}

#[cfg(target_os = "macos")]
fn rasterize(card: &ShareCard, palette: SharePalette) -> Option<RgbaImage> {
    macos::rasterize(card, palette)
}

#[cfg(not(target_os = "macos"))]
fn rasterize(_card: &ShareCard, _palette: SharePalette) -> Option<RgbaImage> {
    None
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use crate::macos::brand_raster::{
        DIRI_LOGO_BASELINE, DIRI_LOGO_CHEVRON, DIRI_LOGO_STROKE, DIRI_LOGO_VB_H, DIRI_LOGO_VB_W,
    };
    use objc2::runtime::AnyObject;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{
        NSBezierPath, NSBitmapFormat, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSFont,
        NSFontAttributeName, NSFontWeightMedium, NSForegroundColorAttributeName, NSGraphicsContext,
        NSLineCapStyle, NSLineJoinStyle, NSStringDrawing,
    };
    use objc2_foundation::{
        NSAttributedStringKey, NSDictionary, NSPoint, NSRect, NSSize, NSString,
    };
    use std::ptr;

    const SCALE: f32 = 2.0;
    const PAD: f32 = 20.0;
    const RADIUS: f32 = 20.0;
    const MARK_H: f32 = 18.0;
    const GRAPH_Y: f32 = 82.0;
    const GRAPH_H: f32 = 118.0;
    const PROVIDER_Y: f32 = 214.0;

    pub(super) fn rasterize(card: &ShareCard, palette: SharePalette) -> Option<RgbaImage> {
        let _mtm = MainThreadMarker::new()?;
        let card_h = card.height();
        let pixel_w = (CARD_W * SCALE).round() as usize;
        let pixel_h = (card_h * SCALE).round() as usize;
        let bitmap = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                ptr::null_mut(),
                pixel_w as isize,
                pixel_h as isize,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                NSBitmapFormat::empty(),
                0,
                32,
            )?
        };
        let bytes_per_row = bitmap.bytesPerRow() as usize;
        let byte_count = bytes_per_row.checked_mul(pixel_h)?;
        let data = bitmap.bitmapData();
        if data.is_null() {
            return None;
        }
        // SAFETY: NSBitmapImageRep owns the plane for the lifetime of `bitmap`.
        let bitmap_bytes = unsafe { std::slice::from_raw_parts_mut(data, byte_count) };
        bitmap_bytes.fill(0);

        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        paint(card, palette, card_h);
        NSGraphicsContext::restoreGraphicsState_class();

        let mut rgba = vec![0_u8; pixel_w * pixel_h * 4];
        for output_y in 0..pixel_h {
            let input_row = &bitmap_bytes[output_y * bytes_per_row..][..pixel_w * 4];
            let output_row = &mut rgba[output_y * pixel_w * 4..][..pixel_w * 4];
            for (source, destination) in input_row
                .chunks_exact(4)
                .zip(output_row.chunks_exact_mut(4))
            {
                // NSBitmapFormat::empty() is RGBA, alpha last.
                let (r, g, b, a) = (source[0], source[1], source[2], source[3]);
                if a == 0 {
                    destination.copy_from_slice(&[0, 0, 0, 0]);
                } else if a == 255 {
                    destination.copy_from_slice(&[r, g, b, a]);
                } else {
                    destination.copy_from_slice(&[
                        ((u16::from(r) * 255) / u16::from(a)) as u8,
                        ((u16::from(g) * 255) / u16::from(a)) as u8,
                        ((u16::from(b) * 255) / u16::from(a)) as u8,
                        a,
                    ]);
                }
            }
        }
        RgbaImage::from_raw(pixel_w as u32, pixel_h as u32, rgba)
    }

    fn paint(card: &ShareCard, palette: SharePalette, card_h: f32) {
        fill_round_rect(
            0.0,
            0.0,
            CARD_W,
            card_h,
            RADIUS,
            opaque(palette.background),
            card_h,
        );
        let mark_w = MARK_H * (DIRI_LOGO_VB_W / DIRI_LOGO_VB_H);
        let brand_font = brand_font_matching(MARK_H);
        let brand_size = measure(BRAND, &brand_font);
        let header_h = MARK_H.max(brand_size.1);
        let mut kicker = card.period().to_owned();
        if let Some(host) = card.host.phrase() {
            kicker = format!("{host} · {kicker}");
        }
        let kicker_font = medium_font(px(12.0));
        let kicker_size = measure(&kicker, &kicker_font);
        draw_text(
            &kicker,
            PAD,
            PAD + (header_h - kicker_size.1) * 0.5,
            &kicker_font,
            palette.secondary,
            card_h,
        );
        let mark_x = CARD_W - PAD - mark_w;
        let brand_x = mark_x - 8.0 - brand_size.0;
        draw_text(
            BRAND,
            brand_x,
            PAD + (header_h - brand_size.1) * 0.5,
            &brand_font,
            palette.primary,
            card_h,
        );
        draw_mark(
            mark_x,
            PAD + (header_h - MARK_H) * 0.5,
            MARK_H,
            palette.primary,
            card_h,
        );

        let hero_font = medium_mono(px(26.0));
        draw_text(&card.hero, PAD, 48.0, &hero_font, palette.primary, card_h);
        if card.tokens {
            let unit_font = medium_font(px(11.0));
            let hero_size = measure(&card.hero, &hero_font);
            draw_text(
                "tokens",
                PAD + hero_size.0 + 6.0,
                48.0,
                &unit_font,
                palette.tertiary,
                card_h,
            );
        }
        let metric_font = medium_font(px(11.0));

        draw_graph(card, palette, card_h);

        let name_font = medium_font(px(13.0));
        let value_font = medium_mono(px(13.0));
        let detail_font = NSFont::systemFontOfSize(px(11.0));
        let mut y = PROVIDER_Y;
        for provider in &card.providers {
            let color = palette.providers[provider.index];
            fill_round_rect(PAD, y + 4.0, 6.0, 6.0, 3.0, color, card_h);
            draw_text(
                provider.name,
                PAD + 12.0,
                y,
                &name_font,
                palette.secondary,
                card_h,
            );
            let value_size = measure(&provider.value, &value_font);
            draw_text(
                &provider.value,
                CARD_W - PAD - value_size.0,
                y,
                &value_font,
                palette.primary,
                card_h,
            );
            draw_text(
                &provider.detail,
                PAD + 12.0,
                y + 16.0,
                &detail_font,
                palette.tertiary,
                card_h,
            );
            let bar_y = y + 34.0;
            let bar_w = CARD_W - PAD * 2.0;
            fill_round_rect(
                PAD,
                bar_y,
                bar_w,
                3.0,
                1.5,
                palette.primary.alpha(0.08),
                card_h,
            );
            let fill =
                (bar_w * provider.share as f32).max(if provider.share > 0.0 { 3.0 } else { 0.0 });
            if fill > 0.0 {
                fill_round_rect(PAD, bar_y, fill, 3.0, 1.5, color, card_h);
            }
            y += 46.0;
        }

        if let Some(models) = &card.models {
            y += 4.0;
            draw_text("Top models", PAD, y, &metric_font, palette.tertiary, card_h);
            y += 18.0;
            let model_font = medium_font(px(12.0));
            let meta_font = medium_mono(px(11.0));
            for model in models {
                draw_text(&model.name, PAD, y, &model_font, palette.primary, card_h);
                let value_size = measure(&model.value, &meta_font);
                let share_size = measure(&model.share, &meta_font);
                let tokens_size = measure(&model.tokens, &meta_font);
                let mut x = CARD_W - PAD;
                x -= tokens_size.0;
                draw_text(&model.tokens, x, y, &meta_font, palette.secondary, card_h);
                x -= 12.0 + share_size.0;
                draw_text(&model.share, x, y, &meta_font, palette.tertiary, card_h);
                x -= 12.0 + value_size.0;
                draw_text(&model.value, x, y, &meta_font, palette.primary, card_h);
                y += 24.0;
            }
        }
    }

    fn draw_graph(card: &ShareCard, palette: SharePalette, card_h: f32) {
        let x = PAD;
        let y = GRAPH_Y;
        let w = CARD_W - PAD * 2.0;
        let h = GRAPH_H;
        fill_round_rect(
            x,
            y + h - 1.0,
            w,
            1.0,
            0.5,
            palette.primary.alpha(0.08),
            card_h,
        );
        let fill_under = card.graph.len() == 1;
        for series in &card.graph {
            let color = series
                .provider
                .map(|index| palette.providers[index])
                .unwrap_or(palette.primary);
            let mut pts: Vec<(f32, f32)> = series
                .points
                .iter()
                .map(|(px, py)| (x + px * w, y + h - py * h))
                .collect();
            if pts.len() == 1 {
                let gy = pts[0].1;
                pts = vec![(x, gy), (x + w, gy)];
            }
            if pts.len() < 2 {
                continue;
            }
            if fill_under {
                fill_series(&pts, y + h, color.alpha(0.16), card_h);
            }
            stroke_series(&pts, color, card_h);
            let last = pts[pts.len() - 1];
            fill_round_rect(last.0 - 3.5, last.1 - 3.5, 7.0, 7.0, 3.5, color, card_h);
        }
    }

    fn fill_series(pts: &[(f32, f32)], base_y: f32, color: Rgba, card_h: f32) {
        let path = NSBezierPath::bezierPath();
        path.moveToPoint(NSPoint::new(px(pts[0].0), appkit_y(base_y, 0.0, card_h)));
        for &(gx, gy) in pts {
            path.lineToPoint(NSPoint::new(px(gx), appkit_y(gy, 0.0, card_h)));
        }
        let last = pts[pts.len() - 1];
        path.lineToPoint(NSPoint::new(px(last.0), appkit_y(base_y, 0.0, card_h)));
        path.closePath();
        ns_color(color).setFill();
        path.fill();
    }

    fn stroke_series(pts: &[(f32, f32)], color: Rgba, card_h: f32) {
        let path = NSBezierPath::bezierPath();
        path.setLineWidth(px(2.0));
        path.setLineCapStyle(NSLineCapStyle::Round);
        path.setLineJoinStyle(NSLineJoinStyle::Round);
        path.moveToPoint(NSPoint::new(px(pts[0].0), appkit_y(pts[0].1, 0.0, card_h)));
        for &(gx, gy) in &pts[1..] {
            path.lineToPoint(NSPoint::new(px(gx), appkit_y(gy, 0.0, card_h)));
        }
        ns_color(color).setStroke();
        path.stroke();
    }

    fn draw_mark(x: f32, y: f32, height: f32, color: Rgba, card_h: f32) {
        let scale = (height * SCALE) / DIRI_LOGO_VB_H;
        let origin_x = x * SCALE;
        let origin_y = (card_h - y - height) * SCALE;
        let map = |mark_x: f32, mark_y: f32| {
            NSPoint::new(
                f64::from(origin_x + mark_x * scale),
                f64::from(origin_y + (DIRI_LOGO_VB_H - mark_y) * scale),
            )
        };
        let strokes = NSBezierPath::bezierPath();
        strokes.setLineWidth(f64::from(DIRI_LOGO_STROKE * scale));
        strokes.setLineCapStyle(NSLineCapStyle::Round);
        strokes.setLineJoinStyle(NSLineJoinStyle::Round);
        let mut chevron = DIRI_LOGO_CHEVRON.iter();
        if let Some(&(mark_x, mark_y)) = chevron.next() {
            strokes.moveToPoint(map(mark_x, mark_y));
        }
        for &(mark_x, mark_y) in chevron {
            strokes.lineToPoint(map(mark_x, mark_y));
        }
        let ((from_x, from_y), (to_x, to_y)) = DIRI_LOGO_BASELINE;
        strokes.moveToPoint(map(from_x, from_y));
        strokes.lineToPoint(map(to_x, to_y));
        ns_color(color).setStroke();
        strokes.stroke();
    }

    fn fill_round_rect(x: f32, y: f32, w: f32, h: f32, radius: f32, color: Rgba, card_h: f32) {
        let rect = NSRect::new(
            NSPoint::new(px(x), appkit_y(y, h, card_h)),
            NSSize::new(px(w), px(h)),
        );
        let path =
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, px(radius), px(radius));
        ns_color(color).setFill();
        path.fill();
    }

    fn draw_text(text: &str, x: f32, y: f32, font: &NSFont, color: Rgba, card_h: f32) {
        let ns = NSString::from_str(text);
        let attrs = attributes(font, color);
        let size = unsafe { ns.sizeWithAttributes(Some(attrs.as_ref())) };
        unsafe {
            ns.drawAtPoint_withAttributes(
                NSPoint::new(px(x), appkit_y(y, size.height as f32 / SCALE, card_h)),
                Some(attrs.as_ref()),
            );
        }
    }

    fn measure(text: &str, font: &NSFont) -> (f32, f32) {
        let ns = NSString::from_str(text);
        let attrs = attributes(font, Rgba::default());
        let size = unsafe { ns.sizeWithAttributes(Some(attrs.as_ref())) };
        (size.width as f32 / SCALE, size.height as f32 / SCALE)
    }

    fn medium_font(size: f64) -> objc2::rc::Retained<NSFont> {
        NSFont::systemFontOfSize_weight(size, unsafe { NSFontWeightMedium })
    }

    fn brand_font_matching(height: f32) -> objc2::rc::Retained<NSFont> {
        let mut size = height;
        for _ in 0..6 {
            let font = medium_font(px(size));
            let measured = measure(BRAND, &font).1;
            if (measured - height).abs() < 0.5 {
                return font;
            }
            size *= height / measured.max(1.0);
        }
        medium_font(px(size))
    }

    fn medium_mono(size: f64) -> objc2::rc::Retained<NSFont> {
        NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe { NSFontWeightMedium })
    }

    fn attributes(
        font: &NSFont,
        color: Rgba,
    ) -> objc2::rc::Retained<NSDictionary<NSAttributedStringKey, AnyObject>> {
        let color = ns_color(color);
        let font_obj: &AnyObject = font.as_ref();
        let color_obj: &AnyObject = color.as_ref();
        unsafe {
            NSDictionary::from_slices(
                &[NSFontAttributeName, NSForegroundColorAttributeName],
                &[font_obj, color_obj],
            )
        }
    }

    fn ns_color(color: Rgba) -> objc2::rc::Retained<NSColor> {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(color.r),
            f64::from(color.g),
            f64::from(color.b),
            f64::from(color.a.clamp(0.0, 1.0)),
        )
    }

    fn opaque(color: Rgba) -> Rgba {
        Rgba { a: 1.0, ..color }
    }

    fn px(value: f32) -> f64 {
        f64::from(value * SCALE)
    }

    fn appkit_y(y_down: f32, height: f32, card_h: f32) -> f64 {
        px(card_h - y_down - height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::UsageHourAgg;
    use crate::usage::dashboard::{UsageDetail, UsageReport};

    fn options(tokens: bool, models: bool) -> ShareOptions {
        ShareOptions {
            tokens,
            individual: false,
            include_models: models,
            theme_id: "dirijor-dark".into(),
        }
    }

    fn report() -> UsageReport {
        let mut report = UsageReport::default();
        report.providers[0] = UsageDetail {
            tokens: UsageHourAgg {
                i: 1_000_000,
                o: 200_000,
                cr: 0,
                cw: 0,
                c: 4.0,
            },
            priced_tokens: 1_200_000,
            ..UsageDetail::default()
        };
        report.providers[1] = UsageDetail {
            tokens: UsageHourAgg {
                i: 800_000,
                o: 100_000,
                cr: 0,
                cw: 0,
                c: 3.0,
            },
            priced_tokens: 900_000,
            ..UsageDetail::default()
        };
        report.providers[2] = UsageDetail {
            tokens: UsageHourAgg {
                i: 400_000,
                o: 50_000,
                cr: 0,
                cw: 0,
                c: 2.0,
            },
            priced_tokens: 450_000,
            ..UsageDetail::default()
        };
        report.total = UsageDetail {
            tokens: UsageHourAgg {
                i: 2_200_000,
                o: 350_000,
                cr: 0,
                cw: 0,
                c: 9.0,
            },
            priced_tokens: 2_550_000,
            ..UsageDetail::default()
        };
        report.models = vec![
            ModelRow {
                model: "claude-opus-4-6".into(),
                provider: 0,
                detail: report.providers[0],
            },
            ModelRow {
                model: "gpt-5.4".into(),
                provider: 1,
                detail: report.providers[1],
            },
            ModelRow {
                model: "composer-2".into(),
                provider: 2,
                detail: report.providers[2],
            },
            ModelRow {
                model: "unused".into(),
                provider: 0,
                detail: UsageDetail::default(),
            },
        ];
        report
    }

    #[test]
    fn caption_names_filters_and_split() {
        let card = ShareCard::from_report(
            &report(),
            30,
            HostLabel::Named("Forge".into()),
            &options(false, false),
            Vec::new(),
        );
        let caption = card.caption();
        assert!(
            caption.contains("diri.sh · Forge · last 30 days · $9.00"),
            "{caption}"
        );
        assert!(
            caption.contains("Claude $4.00 · Codex $3.00 · Cursor $2.00"),
            "{caption}"
        );
        assert!(!caption.to_lowercase().contains("estimate"), "{caption}");
        assert!(!caption.contains("claude-opus"), "{caption}");
        let url = url::Url::parse(&tweet_intent_url(&caption)).unwrap();
        assert_eq!(url.host_str(), Some("x.com"));
        assert_eq!(url.path(), "/intent/tweet");
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "text")
                .map(|(_, value)| value.into_owned())
                .as_deref(),
            Some(caption.as_str())
        );
        assert_eq!(card.file_name(), "diri-usage-30d-cost.png");
        assert_eq!(card.providers[0].detail, "44.4% of cost · 1.2M tokens");
    }

    #[test]
    fn token_mode_and_this_mac_show_up_in_the_caption() {
        let card = ShareCard::from_report(
            &report(),
            1,
            HostLabel::ThisMac,
            &options(true, false),
            Vec::new(),
        );
        let caption = card.caption();
        assert!(caption.contains("this Mac"), "{caption}");
        assert!(caption.contains("last 24 hours"), "{caption}");
        assert!(caption.contains("tokens"), "{caption}");
        assert_eq!(card.file_name(), "diri-usage-24h-tokens.png");
        assert!(
            card.providers[0].detail.contains("% of tokens"),
            "{}",
            card.providers[0].detail
        );
    }

    #[test]
    fn zero_cost_providers_drop_out_of_the_card() {
        let mut report = report();
        report.providers[2] = UsageDetail::default();
        report.total.tokens.c = 7.0;
        let card = ShareCard::from_report(
            &report,
            7,
            HostLabel::ThisMac,
            &options(false, false),
            Vec::new(),
        );
        assert_eq!(
            card.providers
                .iter()
                .map(|provider| provider.name)
                .collect::<Vec<_>>(),
            ["Claude", "Codex"]
        );
    }

    #[test]
    fn top_models_are_the_three_highest_and_skip_empty() {
        let card = ShareCard::from_report(
            &report(),
            7,
            HostLabel::Hidden,
            &options(false, true),
            Vec::new(),
        );
        let models = card.models.as_ref().expect("models on");
        assert_eq!(
            models
                .iter()
                .map(|model| model.name.as_str())
                .collect::<Vec<_>>(),
            ["claude-opus-4-6", "gpt-5.4", "composer-2"]
        );
        assert!(
            card.caption().contains("claude-opus-4-6 $4.00"),
            "{}",
            card.caption()
        );
    }

    #[test]
    fn png_roundtrip_keeps_the_png_signature() {
        let image = RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        let png = encode_png(&image).expect("png");
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        let decoded = image::load_from_memory(&png).expect("decode").to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }
}
