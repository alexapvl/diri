//! Usage settings use the existing ledger and settings shell.
use super::usage_chart::{self, ChartSample};
use super::*;
use crate::usage::dashboard::{UsageHistory, UsageReport, date_label, hour_label};
use crate::usage::{UsageFormat, UsageSnapshot};
use diri_ui::Ink;
use gpui::{CursorStyle, MouseMoveEvent, canvas, relative};
use std::cell::Cell;
use std::rc::Rc;

const PROVIDERS: [&str; 3] = ["Claude Code", "Codex", "Cursor"];
const SERIES_LABELS: [&str; 3] = ["Claude", "Codex", "Cursor"];
const SERIES_HOVER_PILL: u8 = 1;
const SERIES_HOVER_MENU: u8 = 2;
fn provider_color(provider: usize, colors: SemanticColors) -> Rgba {
    match provider {
        0 => rgba(0xcf876dff),
        2 => rgba(0x6d8fcfff),
        _ => colors.secondary,
    }
}

impl UtilitySurfaces {
    pub(crate) fn set_usage(&mut self, usage: UsageSnapshot, cx: &mut Context<Self>) {
        if self
            .usage_host
            .as_deref()
            .is_some_and(|id| !id.is_empty() && !usage.remote.iter().any(|host| host.host == id))
        {
            self.usage_host = None;
        }
        self.usage = usage;
        if self.surface == Surface::Settings && self.settings_tab == SettingsTab::Usage {
            cx.notify();
        }
    }

    pub(super) fn usage_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.settings_colors();
        if self.usage.updated_at == 0 {
            return settings_page("Usage", div().flex().flex_col().gap(px(8.0)).py(px(24.0))
                .child(label("Reading local usage…", 14.0, colors.primary))
                .child(label("Preparing costs and token history from local Claude Code and Codex transcripts, plus billed Cursor usage when signed in.", 12.0, colors.secondary)), colors).into_any_element();
        }
        let now = self
            .usage
            .remote
            .iter()
            .filter_map(|host| host.data.as_ref().map(|data| data.collected_at))
            .fold(self.usage.updated_at, i64::max);
        let history = self.usage.history_for_source(self.usage_host.as_deref());
        let compare = history.compare(now, self.usage_days);
        let report = &compare.current;
        let chart_providers = self.chart_provider_samples(&history, now);
        let total = report.total.totals();
        let loaded = self.usage.updated_at > 0;
        let mut ranges = div().flex().gap(px(3.0));
        for days in [1, 7, 30, 90] {
            ranges = ranges.child(
                usage_control(
                    format!("usage-range-{days}"),
                    range_label(days),
                    self.usage_days == days,
                    colors,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_usage_days(days, window, cx);
                })),
            );
        }
        let tokens = self.usage_tokens;
        let mut providers = div().flex().flex_col().gap(px(12.0));
        for (index, provider) in report.providers.iter().enumerate() {
            let provider_tokens = provider.totals().total_tokens();
            let share = if tokens {
                ratio(provider_tokens as f64, total.total_tokens() as f64)
            } else {
                ratio(provider.tokens.c, total.cost)
            };
            providers = providers.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap(px(12.0))
                            .child(provider_label(index, colors))
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap(px(6.0))
                                    .child(if tokens {
                                        self.usage_numbers.show(
                                            format!("provider-{index}"),
                                            provider_tokens as f64,
                                            UsageFormat::tokens(provider_tokens),
                                            12.0,
                                            colors.primary,
                                            FontWeight::NORMAL,
                                        )
                                    } else {
                                        self.usage_numbers.show(
                                            format!("provider-{index}"),
                                            provider.tokens.c,
                                            money(provider.tokens.c),
                                            12.0,
                                            colors.primary,
                                            FontWeight::NORMAL,
                                        )
                                    })
                                    .when_some(
                                        if tokens {
                                            compare.provider_tokens_change(index)
                                        } else {
                                            compare.provider_cost_change(index)
                                        },
                                        |row, change| {
                                            row.child(change_delta(
                                                &self.usage_numbers,
                                                format!("delta-provider-{index}"),
                                                change,
                                                colors,
                                            ))
                                        },
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h(px(3.0))
                            .w_full()
                            .rounded(px(2.0))
                            .bg(colors.primary.alpha(0.06))
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(share as f32))
                                    .rounded(px(2.0))
                                    .bg(provider_color(index, colors)),
                            ),
                    )
                    .child(label(
                        if tokens {
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
                        11.0,
                        colors.tertiary,
                    )),
            );
        }
        let hero = div()
            .flex()
            .flex_wrap()
            .items_stretch()
            .gap(px(28.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(240.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(value_with_delta(
                                &self.usage_numbers,
                                "delta-hero",
                                if !loaded {
                                    label("—", 34.0, colors.primary)
                                        .font_weight(FontWeight::MEDIUM)
                                        .line_height(px(34.0))
                                        .into_any_element()
                                } else if tokens {
                                    self.usage_numbers.show(
                                        "hero",
                                        total.total_tokens() as f64,
                                        UsageFormat::tokens(total.total_tokens()),
                                        34.0,
                                        colors.primary,
                                        FontWeight::MEDIUM,
                                    )
                                } else {
                                    self.usage_numbers.show(
                                        "hero",
                                        total.cost,
                                        money(total.cost),
                                        34.0,
                                        colors.primary,
                                        FontWeight::MEDIUM,
                                    )
                                },
                                if tokens {
                                    compare.processed_tokens_change()
                                } else {
                                    compare.cost_change()
                                },
                                colors,
                            ))
                            .child(label(
                                "Claude and Codex at model rates. Cursor is billed usage.",
                                11.0,
                                colors.tertiary,
                            )),
                    )
                    .child(providers.mt(px(16.0))),
            )
            .child(
                self.usage_chart(&chart_providers, colors, cx)
                    .into_any_element(),
            );
        let input = total.input_tokens + total.cache_read_tokens + total.cache_write_tokens;
        let metrics = div()
            .flex()
            .flex_wrap()
            .gap(px(1.0))
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(colors.primary.alpha(0.06))
            .child(metric(
                &self.usage_numbers,
                "delta-metric-processed",
                "Processed tokens",
                self.usage_numbers.show(
                    "metric-processed",
                    total.total_tokens() as f64,
                    UsageFormat::tokens(total.total_tokens()),
                    20.0,
                    colors.primary,
                    FontWeight::NORMAL,
                ),
                compare.processed_tokens_change(),
                format!(
                    "{} per active day",
                    UsageFormat::tokens(total.total_tokens() / report.active_days.max(1) as i64)
                ),
                colors,
            ))
            .child(metric(
                &self.usage_numbers,
                "delta-metric-cached",
                "Cached input",
                self.usage_numbers.show(
                    "metric-cached",
                    total.cache_read_tokens as f64,
                    UsageFormat::tokens(total.cache_read_tokens),
                    20.0,
                    colors.primary,
                    FontWeight::NORMAL,
                ),
                compare.cached_input_change(),
                format!(
                    "{:.1}% of input",
                    ratio(total.cache_read_tokens as f64, input as f64) * 100.0
                ),
                colors,
            ))
            .child(metric(
                &self.usage_numbers,
                "delta-metric-uncached",
                "Uncached input",
                self.usage_numbers.show(
                    "metric-uncached",
                    total.input_tokens as f64,
                    UsageFormat::tokens(total.input_tokens),
                    20.0,
                    colors.primary,
                    FontWeight::NORMAL,
                ),
                compare.uncached_input_change(),
                format!(
                    "{} cache writes",
                    UsageFormat::tokens(total.cache_write_tokens)
                ),
                colors,
            ))
            .child(metric(
                &self.usage_numbers,
                "delta-metric-output",
                "Output",
                self.usage_numbers.show(
                    "metric-output",
                    total.output_tokens as f64,
                    UsageFormat::tokens(total.output_tokens),
                    20.0,
                    colors.primary,
                    FontWeight::NORMAL,
                ),
                compare.output_change(),
                format!(
                    "{} reasoning reported",
                    UsageFormat::tokens(report.total.reasoning)
                ),
                colors,
            ))
            .child(metric(
                &self.usage_numbers,
                "delta-metric-savings",
                "Cache read savings",
                self.usage_numbers.show(
                    "metric-savings",
                    report.total.read_savings,
                    money(report.total.read_savings),
                    20.0,
                    colors.primary,
                    FontWeight::NORMAL,
                ),
                compare.read_savings_change(),
                "Estimated · excludes writes".into(),
                colors,
            ));
        let mut content = div().flex().flex_col().gap(px(24.0)).child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(label(
                    if tokens {
                        "Processed tokens · compared with the previous period of the same length."
                    } else {
                        "Estimated API cost · compared with the previous period of the same length."
                    },
                    12.0,
                    colors.secondary,
                ))
                .child(ranges),
        );
        if !self.usage.remote.is_empty() {
            let mut sources = div()
                .flex()
                .flex_wrap()
                .gap(px(4.0))
                .child(
                    usage_control(
                        "usage-source-all",
                        "All machines",
                        self.usage_host.is_none(),
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.usage_host = None;
                        cx.notify();
                    })),
                )
                .child(
                    usage_control(
                        "usage-source-local",
                        "This Mac",
                        self.usage_host.as_deref() == Some(""),
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.usage_host = Some(String::new());
                        cx.notify();
                    })),
                );
            let mut status = div().flex().flex_col().gap(px(4.0));
            for host in &self.usage.remote {
                let id = host.host.clone();
                sources = sources.child(
                    usage_control(
                        format!("usage-source-{id}"),
                        host.name.clone(),
                        self.usage_host.as_deref() == Some(id.as_str()),
                        colors,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.usage_host = Some(id.clone());
                        cx.notify();
                    })),
                );
                if self.usage_host.as_deref().is_none_or(|id| id == host.host) {
                    use crate::usage::RemoteUsageStatus;
                    let last = host.data.as_ref().map(|data| {
                        let time = data.collected_at;
                        format!(
                            "{} {:02}:{:02} UTC",
                            date_label(time.div_euclid(86_400)),
                            time.rem_euclid(86_400) / 3_600,
                            time.rem_euclid(3_600) / 60
                        )
                    });
                    let message = match (host.status, last) {
                        (RemoteUsageStatus::Loading, Some(last)) => format!("Updating · cached through {last}"),
                        (RemoteUsageStatus::Loading, None) => "Reading remote usage…".to_owned(),
                        (RemoteUsageStatus::Unavailable, Some(last)) => format!("Unavailable · showing usage saved at {last}"),
                        (RemoteUsageStatus::Unavailable, None) => "Usage unavailable · retries every 5 minutes; check the connection in Remote settings".to_owned(),
                        (RemoteUsageStatus::Ready, Some(last)) => format!("Updated {last}"),
                        (RemoteUsageStatus::Ready, None) => "No transcript history".to_owned(),
                    };
                    status = status.child(label(
                        format!("{} · {message}", host.name),
                        11.0,
                        colors.secondary,
                    ));
                }
            }
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(sources)
                    .child(status),
            );
        }
        if loaded && self.usage_days != 1 && total.total_tokens() == 0 {
            content = content.child(div().p(px(20.0)).rounded(px(8.0)).bg(colors.primary.alpha(0.035)).flex().flex_col().gap(px(6.0))
                .child(label("Your usage starts with a conversation", 14.0, colors.primary))
                .child(label("Available Claude Code and Codex transcripts appear automatically, alongside signed-in Cursor usage on this Mac.", 12.0, colors.secondary))
                .child(label("Try a longer date range to see earlier activity.", 12.0, colors.secondary)));
        }
        content = content.child(hero).child(metrics).child(self.usage_breakdown(report, colors))
            .child(div().flex().flex_col().gap(px(7.0))
                .child(label("About these estimates", 12.0, colors.primary).font_weight(FontWeight::MEDIUM))
                .child(label(format!("{:.1}% of tokens priced · {} unpriced tokens", ratio(report.total.priced_tokens as f64, total.total_tokens() as f64) * 100.0, UsageFormat::tokens(total.total_tokens() - report.total.priced_tokens)), 11.0, colors.secondary))
                .child(label("Uses Diri’s bundled model rates for Claude and Codex. Cursor costs come from billed dashboard events. Unpriced Claude/Codex usage is excluded from cost. Cache read savings compare cached reads with uncached input rates; cache write premiums are excluded.", 11.0, colors.tertiary))
                .child(label("Includes local and remote Claude Code and Codex transcripts, including sessions outside Diri, plus billed Cursor usage on this Mac. Remote machines refresh every 5 minutes over SSH; unavailable machines keep their last saved totals.", 11.0, colors.tertiary)));
        settings_page("Usage", content, colors).into_any_element()
    }

    fn usage_now(&self) -> i64 {
        self.usage
            .remote
            .iter()
            .filter_map(|host| host.data.as_ref().map(|data| data.collected_at))
            .fold(self.usage.updated_at, i64::max)
    }

    fn chart_provider_samples(&self, history: &UsageHistory, now: i64) -> [Vec<ChartSample>; 3] {
        let mut providers = [Vec::new(), Vec::new(), Vec::new()];
        for (hour, details) in history.hourly_provider_totals(now, 90) {
            for (index, detail) in details.into_iter().enumerate() {
                providers[index].push(ChartSample {
                    time: hour,
                    value: if self.usage_tokens {
                        detail.totals().total_tokens() as f64
                    } else {
                        detail.tokens.c
                    },
                });
            }
        }
        providers
    }

    fn chart_visible_series(
        &self,
        providers: &[Vec<ChartSample>; 3],
        window_days: f32,
        end: i64,
    ) -> Vec<Vec<ChartSample>> {
        match self.usage_chart_split {
            None => vec![usage_chart::displayed_series(
                &usage_chart::combined_series(providers),
                window_days,
                end,
            )],
            Some(visible) => (0..3)
                .filter(|&index| visible[index])
                .map(|index| usage_chart::displayed_series(&providers[index], window_days, end))
                .collect(),
        }
    }

    fn retarget_chart_range(
        &mut self,
        providers: &[Vec<ChartSample>; 3],
        current_days: f32,
        target_days: f32,
        end: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reduce = cx.reduce_motion();
        let current_range =
            usage_chart::series_range(&self.chart_visible_series(providers, current_days, end));
        let target_range =
            usage_chart::series_range(&self.chart_visible_series(providers, target_days, end));
        if reduce {
            self.usage_chart_window.snap(target_days, target_range);
            self.usage_chart_tick = None;
        } else {
            self.usage_chart_window
                .begin(current_days, target_days, current_range, target_range);
            self.ensure_chart_tick(window, cx);
        }
    }

    fn usage_series_menu_open(&self) -> bool {
        self.usage_series_hover != 0 || self.usage_series_menu_close.is_some()
    }

    fn hover_usage_series_menu(&mut self, layer: u8, hovered: bool, cx: &mut Context<Self>) {
        let before = self.usage_series_menu_open();
        if hovered {
            self.usage_series_hover |= layer;
            self.usage_series_menu_close = None;
        } else {
            self.usage_series_hover &= !layer;
            if self.usage_series_hover == 0 && self.usage_series_menu_close.is_none() {
                self.usage_series_menu_close = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(140))
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        this.usage_series_menu_close = None;
                        if this.usage_series_hover == 0 {
                            cx.notify();
                        }
                    });
                }));
            }
        }
        if before != self.usage_series_menu_open() {
            cx.notify();
        }
    }

    fn set_usage_chart_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.usage_series_hover = 0;
        self.usage_series_menu_close = None;
        if self.usage_chart_split.is_none() {
            cx.notify();
            return;
        }
        let reduce = cx.reduce_motion();
        let history = self.usage.history_for_source(self.usage_host.as_deref());
        let providers = self.chart_provider_samples(&history, self.usage_now());
        let end = providers[0].last().map(|sample| sample.time).unwrap_or(0);
        let current_days = self.usage_chart_window.displayed_days(reduce);
        let current_range =
            usage_chart::series_range(&self.chart_visible_series(&providers, current_days, end));
        self.usage_chart_split = None;
        let target_range =
            usage_chart::series_range(&self.chart_visible_series(&providers, current_days, end));
        if reduce {
            self.usage_chart_window.snap(current_days, target_range);
            self.usage_chart_tick = None;
        } else {
            self.usage_chart_window
                .begin(current_days, current_days, current_range, target_range);
            self.ensure_chart_tick(window, cx);
        }
        cx.notify();
    }

    fn set_usage_chart_individual(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.usage_chart_split.is_some() {
            return;
        }
        let reduce = cx.reduce_motion();
        let history = self.usage.history_for_source(self.usage_host.as_deref());
        let providers = self.chart_provider_samples(&history, self.usage_now());
        let end = providers[0].last().map(|sample| sample.time).unwrap_or(0);
        let current_days = self.usage_chart_window.displayed_days(reduce);
        let current_range =
            usage_chart::series_range(&self.chart_visible_series(&providers, current_days, end));
        self.usage_chart_split = Some([true; 3]);
        let target_range =
            usage_chart::series_range(&self.chart_visible_series(&providers, current_days, end));
        if reduce {
            self.usage_chart_window.snap(current_days, target_range);
            self.usage_chart_tick = None;
        } else {
            self.usage_chart_window
                .begin(current_days, current_days, current_range, target_range);
            self.ensure_chart_tick(window, cx);
        }
        cx.notify();
    }

    fn toggle_usage_chart_provider(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reduce = cx.reduce_motion();
        let history = self.usage.history_for_source(self.usage_host.as_deref());
        let providers = self.chart_provider_samples(&history, self.usage_now());
        let end = providers[0].last().map(|sample| sample.time).unwrap_or(0);
        let current_days = self.usage_chart_window.displayed_days(reduce);
        let current_range =
            usage_chart::series_range(&self.chart_visible_series(&providers, current_days, end));
        match self.usage_chart_split {
            None => {
                let mut visible = [false; 3];
                visible[index] = true;
                self.usage_chart_split = Some(visible);
            }
            Some(ref mut visible) => {
                if visible[index] {
                    if visible.iter().filter(|&&on| on).count() <= 1 {
                        return;
                    }
                    visible[index] = false;
                } else {
                    visible[index] = true;
                }
            }
        }
        let target_range =
            usage_chart::series_range(&self.chart_visible_series(&providers, current_days, end));
        if reduce {
            self.usage_chart_window.snap(current_days, target_range);
            self.usage_chart_tick = None;
        } else {
            self.usage_chart_window
                .begin(current_days, current_days, current_range, target_range);
            self.ensure_chart_tick(window, cx);
        }
        cx.notify();
    }

    fn set_usage_days(&mut self, days: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.usage_days == days {
            return;
        }
        let reduce = cx.reduce_motion();
        let history = self.usage.history_for_source(self.usage_host.as_deref());
        let providers = self.chart_provider_samples(&history, self.usage_now());
        let end = providers[0].last().map(|sample| sample.time).unwrap_or(0);
        let current_days = self.usage_chart_window.displayed_days(reduce);
        self.retarget_chart_range(&providers, current_days, days as f32, end, window, cx);
        self.usage_days = days;
        cx.notify();
    }

    pub(super) fn ensure_chart_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.usage_chart_tick.is_some() {
            return;
        }
        self.usage_chart_tick = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let done = this
                    .update_in(cx, |this, _, cx| {
                        cx.notify();
                        this.usage_chart_window.finished() && !this.usage_numbers.running()
                    })
                    .unwrap_or(true);
                if done {
                    let _ = this.update_in(cx, |this, _, _| this.usage_chart_tick = None);
                    break;
                }
            }
        }));
    }

    fn usage_chart(
        &self,
        providers: &[Vec<ChartSample>; 3],
        colors: SemanticColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tokens = self.usage_tokens;
        let reduce = cx.reduce_motion();
        let end = providers[0].last().map(|sample| sample.time).unwrap_or(0);
        let window_days = self.usage_chart_window.displayed_days(reduce);
        let hours = usage_chart::window_hours(window_days);
        let series = self.chart_visible_series(providers, window_days, end);
        let line_colors: Vec<Rgba> = match self.usage_chart_split {
            None => vec![colors.primary],
            Some(visible) => (0..3)
                .filter(|&index| visible[index])
                .map(|index| provider_color(index, colors))
                .collect(),
        };
        let line_labels: Vec<&'static str> = match self.usage_chart_split {
            None => Vec::new(),
            Some(visible) => (0..3)
                .filter(|&index| visible[index])
                .map(|index| SERIES_LABELS[index])
                .collect(),
        };
        let computed_range = usage_chart::series_range(&series);
        let (y_min, y_max) = self
            .usage_chart_window
            .displayed_range(computed_range, reduce);
        let first_time = series
            .first()
            .and_then(|line| line.first())
            .map(|sample| sample.time)
            .unwrap_or(end);
        let last_time = series
            .first()
            .and_then(|line| line.last())
            .map(|sample| sample.time)
            .unwrap_or(end);
        let axis_label = |hour: i64| {
            if window_days < 2.0 {
                hour_label(hour)
            } else {
                date_label(hour.div_euclid(24))
            }
        };
        let scrub = self.usage_scrub.map(|(t, _, _)| t);
        let left = end as f32 + 1.0 - hours;
        let format_value = |value: f64| {
            if tokens {
                UsageFormat::tokens(value as i64)
            } else {
                money(value)
            }
        };
        let time_label = |hour: i64| {
            if window_days < 2.0 {
                hour_label(hour)
            } else {
                date_label(hour.div_euclid(24))
            }
        };
        let scrub_tip = self.usage_scrub.and_then(|(t, width, _)| {
            let time = left + t * hours;
            let mut hour = None;
            let mut parts = Vec::new();
            if line_labels.is_empty() {
                let line = series.first()?;
                let (at, value) = usage_chart::interpolate_at(line, time)?;
                hour = Some(at);
                parts.push(format_value(value));
            } else {
                for (line, name) in series.iter().zip(line_labels.iter()) {
                    if let Some((at, value)) = usage_chart::interpolate_at(line, time) {
                        hour = Some(at);
                        parts.push(format!("{name} {}", format_value(value)));
                    }
                }
            }
            let text = format!("{} · {}", parts.join(" · "), time_label(hour?));
            Some((t.clamp(0.0, 1.0) * width, width, text))
        });
        let bounds_slot = Rc::new(Cell::new(None::<Bounds<Pixels>>));
        let paint_lines: Vec<(Vec<ChartSample>, Rgba)> =
            series.into_iter().zip(line_colors).collect();
        let split = self.usage_chart_split;
        let chart = div()
            .id("usage-line")
            .relative()
            .flex_1()
            .min_h(px(188.0))
            .w_full()
            .cursor(CursorStyle::Crosshair)
            .on_mouse_move(cx.listener({
                let bounds_slot = Rc::clone(&bounds_slot);
                move |this, event: &MouseMoveEvent, _, cx| {
                    let Some(bounds) = bounds_slot.get() else {
                        return;
                    };
                    let width = f32::from(bounds.size.width).max(1.0);
                    let height = f32::from(bounds.size.height).max(1.0);
                    let t = ((f32::from(event.position.x) - f32::from(bounds.origin.x)) / width)
                        .clamp(0.0, 1.0);
                    this.usage_scrub = Some((t, width, height));
                    cx.notify();
                }
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered && this.usage_scrub.take().is_some() {
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    {
                        let bounds_slot = Rc::clone(&bounds_slot);
                        move |bounds, _, _| bounds_slot.set(Some(bounds))
                    },
                    move |bounds, _, window, _| {
                        usage_chart::paint(
                            window,
                            bounds,
                            &paint_lines,
                            window_days,
                            y_min,
                            y_max,
                            end,
                            scrub,
                        );
                    },
                )
                .absolute()
                .inset_0(),
            )
            .when_some(scrub_tip, |chart, (x, width, text)| {
                let tip_w = crate::number_flow::tabular_width(&text, 13.0).min(width - 8.0);
                let left = (x - tip_w * 0.5).clamp(0.0, (width - tip_w).max(0.0));
                chart.child(div().absolute().left(px(left)).top(px(0.0)).child(
                    crate::number_flow::tabular(text, 13.0, colors.primary, FontWeight::NORMAL),
                ))
            });
        let series_chips = div()
            .flex()
            .gap(px(3.0))
            .child(
                usage_control("usage-series-all", "All", split.is_none(), colors).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.set_usage_chart_all(window, cx);
                    }),
                ),
            )
            .child(
                div()
                    .id("usage-series-individual-wrap")
                    .relative()
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.hover_usage_series_menu(SERIES_HOVER_PILL, *hovered, cx);
                    }))
                    .child(
                        usage_control_frame("usage-series-individual", split.is_some(), colors)
                            .gap(px(4.0))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_usage_chart_individual(window, cx);
                            }))
                            .child("Individual")
                            .child(sf_symbol(
                                if self.usage_series_menu_open() {
                                    "chevron.up"
                                } else {
                                    "chevron.down"
                                },
                                8.0,
                                colors.tertiary,
                            )),
                    )
                    .when(self.usage_series_menu_open(), |wrap| {
                        wrap.child(series_provider_menu(split, colors, cx))
                    }),
            );
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(240.0))
            .child(
                div()
                    .h(px(27.0))
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap(px(10.0))
                    .child(series_chips)
                    .child(
                        div()
                            .flex()
                            .gap(px(3.0))
                            .child(
                                usage_control("usage-cost", "Cost", !tokens, colors).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.usage_tokens = false;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                usage_control("usage-tokens", "Tokens", tokens, colors).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.usage_tokens = true;
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .flex_1()
                    .min_h(px(188.0))
                    .w_full()
                    .child(chart),
            )
            .child(
                div()
                    .mt(px(5.0))
                    .flex()
                    .justify_between()
                    .child(label(axis_label(first_time), 10.0, colors.tertiary))
                    .child(label(axis_label(last_time), 10.0, colors.tertiary)),
            )
    }

    fn usage_breakdown(&self, report: &UsageReport, colors: SemanticColors) -> impl IntoElement {
        let mut table = div().flex().flex_col().child(table_row(
            "",
            "Model",
            label("Cost", 12.0, colors.primary).into_any_element(),
            label("Share", 12.0, colors.tertiary).into_any_element(),
            label("Tokens", 12.0, colors.secondary).into_any_element(),
            colors,
        ));
        for row in &report.models {
            let total = row.detail.totals();
            table = table.child(table_row(
                PROVIDERS[row.provider],
                &row.model,
                if row.detail.priced_tokens == 0 {
                    label("Unpriced", 12.0, colors.primary).into_any_element()
                } else {
                    label(money(total.cost), 12.0, colors.primary).into_any_element()
                },
                label(
                    format!("{:.1}%", ratio(total.cost, report.total.tokens.c) * 100.0),
                    12.0,
                    colors.tertiary,
                )
                .into_any_element(),
                label(
                    UsageFormat::tokens(total.total_tokens()),
                    12.0,
                    colors.secondary,
                )
                .into_any_element(),
                colors,
            ));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(label("Breakdown", 13.0, colors.primary).font_weight(FontWeight::MEDIUM))
            .child(table)
    }
}

fn range_label(days: usize) -> SharedString {
    match days {
        1 => "24h".into(),
        7 => "7d".into(),
        30 => "1M".into(),
        90 => "3M".into(),
        days => format!("{days}d").into(),
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
fn change_delta(
    numbers: &crate::number_flow::Bank,
    id: impl Into<String>,
    change: f64,
    colors: SemanticColors,
) -> gpui::Div {
    let pct = (change * 100.0).round() as i64;
    if pct == 0 {
        return label("Same", 11.0, colors.secondary).line_height(px(11.0));
    }
    let (mark, amount, color) = if pct > 0 {
        ("▲", pct, Ink::FRESH)
    } else {
        ("▼", -pct, Ink::DANGER)
    };
    div()
        .flex()
        .items_start()
        .gap(px(3.0))
        .line_height(px(11.0))
        .child(label(mark, 9.0, color).line_height(px(11.0)))
        .child(numbers.show(
            id,
            amount as f64,
            format!("{amount}%"),
            11.0,
            color,
            FontWeight::NORMAL,
        ))
}
fn value_with_delta(
    numbers: &crate::number_flow::Bank,
    id: impl Into<String>,
    value: impl IntoElement,
    delta: Option<f64>,
    colors: SemanticColors,
) -> gpui::Div {
    div()
        .flex()
        .items_start()
        .gap(px(6.0))
        .child(value)
        .when_some(delta, |row, change| {
            row.child(change_delta(numbers, id, change, colors))
        })
}
fn label(text: impl Into<SharedString>, size: f32, color: Rgba) -> gpui::Div {
    div()
        .text_size(px(size))
        .text_color(color)
        .child(text.into())
}
fn usage_control(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    selected: bool,
    colors: SemanticColors,
) -> gpui::Stateful<gpui::Div> {
    usage_control_frame(id, selected, colors).child(text.into())
}
fn series_provider_menu(
    split: Option<[bool; 3]>,
    colors: SemanticColors,
    cx: &mut Context<UtilitySurfaces>,
) -> impl IntoElement {
    let mut items = div().flex().flex_col().p(px(4.0)).w(px(148.0));
    for index in 0..3 {
        let selected = split.is_some_and(|visible| visible[index]);
        items = items.child(
            div()
                .id(SharedString::from(format!("usage-series-{index}")))
                .h(px(Metrics::ROW_HEIGHT))
                .px(px(8.0))
                .rounded(px(Radius::ROW))
                .flex()
                .items_center()
                .gap(px(8.0))
                .bg(Fill::selected(colors, selected))
                .cursor_pointer()
                .hover(move |style| style.bg(colors.primary.alpha(0.08)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.toggle_usage_chart_provider(index, window, cx);
                }))
                .child(
                    div()
                        .size(px(6.0))
                        .rounded(px(3.0))
                        .bg(provider_color(index, colors)),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .child(SERIES_LABELS[index]),
                )
                .when(selected, |row| {
                    row.child(sf_symbol("checkmark", 10.0, colors.secondary))
                }),
        );
    }
    deferred(
        div()
            .id("usage-series-menu")
            .absolute()
            .top(px(27.0))
            .left_0()
            .pt(px(4.0))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hover_usage_series_menu(SERIES_HOVER_MENU, *hovered, cx);
            }))
            .child(
                FloatingSurface::new(colors, items)
                    .animate_entry(false)
                    .radius(8.0),
            ),
    )
    .with_priority(5)
}
fn usage_control_frame(
    id: impl Into<SharedString>,
    selected: bool,
    colors: SemanticColors,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .debug_selector(move || id.to_string())
        .px(px(10.0))
        .h(px(27.0))
        .flex()
        .items_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(colors.primary.alpha(if selected { 0.1 } else { 0.0 }))
        .bg(colors.primary.alpha(if selected { 0.065 } else { 0.0 }))
        .text_size(px(11.0))
        .text_color(if selected {
            colors.primary
        } else {
            colors.secondary
        })
        .cursor_pointer()
        .hover(|style| style.bg(colors.primary.alpha(0.09)))
        .active(|style| style.bg(colors.primary.alpha(0.13)))
}
fn provider_label(provider: usize, colors: SemanticColors) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .size(px(6.0))
                .rounded(px(3.0))
                .bg(provider_color(provider, colors)),
        )
        .child(label(PROVIDERS[provider], 11.0, colors.secondary))
}
fn metric(
    numbers: &crate::number_flow::Bank,
    id: &'static str,
    title: &str,
    value: impl IntoElement,
    delta: Option<f64>,
    detail: String,
    colors: SemanticColors,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .p(px(12.0))
        .w(px(170.0))
        .flex_grow(1.0)
        .bg(colors.background)
        .child(label(title.to_owned(), 11.0, colors.secondary))
        .child(value_with_delta(numbers, id, value, delta, colors))
        .child(label(detail, 10.0, colors.tertiary))
}
fn table_row(
    provider: &str,
    name: &str,
    cost: impl IntoElement,
    share: impl IntoElement,
    tokens: impl IntoElement,
    colors: SemanticColors,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .min_h(px(39.0))
        .py(px(7.0))
        .border_b_1()
        .border_color(colors.primary.alpha(0.055))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(label(name.to_owned(), 12.0, colors.primary).truncate())
                .when(!provider.is_empty(), |row| {
                    row.child(label(provider.to_owned(), 10.0, colors.tertiary))
                }),
        )
        .child(div().w(px(85.0)).flex().justify_end().child(cost))
        .child(div().w(px(54.0)).flex().justify_end().child(share))
        .child(div().w(px(64.0)).flex().justify_end().child(tokens))
}
