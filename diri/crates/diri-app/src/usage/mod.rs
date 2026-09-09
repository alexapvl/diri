//! Incremental usage accounting for Claude Code, Codex, and Cursor.
//!
//! Claude and Codex costs come from local transcripts. Cursor usage is fetched
//! from Cursor's dashboard API using the signed-in IDE/CLI session.

mod cache;
mod cursor;
mod fleet;
mod model;
mod parser;
mod pricing;
mod store;
mod timestamp;
mod watcher;

pub(crate) use cursor::merge_cursor_usage;
pub(crate) use fleet::merge_fleet_usage;
pub use model::{ProviderUsage, UsageHourAgg, UsageSnapshot, UsageTotals};
pub use pricing::PRICING_ENTRY_COUNT;
pub use store::{
    Clock, ClockReading, RefreshStats, ScanPaths, SystemClock, UsageFormat, UsageProvider,
    UsageStore,
};
pub(crate) use watcher::{TranscriptInvalidation, TranscriptWatcher};

#[cfg(test)]
mod tests;
