//! Engine-owned causal attention state. Additive to SessionRecord; independent
//! of the Remote Helper protocol and terminal presentation.
use serde::{Deserialize, Serialize};

use crate::{DateMillis, NeedsInputDetail};

pub const ATTENTION_VERSION: u32 = 1;
pub const EVENT_LIMIT: usize = 200;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionState {
    pub version: u32,
    /// Random namespace persisted before the first observable event.
    pub epoch: String,
    pub sequence: u64,
    pub turn: u64,
    pub working: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateMillis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_native_completion: Option<String>,
    /// Only bounded native ids and tool names; no arguments or tool payloads.
    #[serde(default)]
    pub active_tools: std::collections::BTreeMap<String, String>,
    /// Bounded event history; active requests are never evicted.
    pub events: Vec<AttentionEvent>,
    /// Native identities already observed, retained independently of display rows.
    #[serde(default)]
    pub native_requests: std::collections::BTreeMap<String, u64>,
    #[serde(default)]
    pub native_completions: std::collections::BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttentionKind {
    Request,
    Completion,
    Failure,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionEvent {
    pub sequence: u64,
    pub turn: u64,
    pub kind: AttentionKind,
    pub occurred_at: DateMillis,
    pub resolved: bool,
    pub blocking: bool,
    pub detail: Option<NeedsInputDetail>,
}

impl AttentionState {
    pub fn event_id(&self, event: &AttentionEvent) -> String {
        format!("attention-v1-{}-{}", self.epoch, event.sequence)
    }

    pub fn active_requests(&self) -> impl Iterator<Item = &AttentionEvent> {
        self.events
            .iter()
            .filter(|event| event.kind == AttentionKind::Request && !event.resolved)
    }

    /// The turn that finished between `before` and `self`, if any. A
    /// restarted history (new epoch) or a still-working Agent is not a
    /// completion, so a reconnect never reports every session's last reply.
    #[must_use]
    pub fn completed_turn_since(&self, before: &AttentionState) -> Option<u64> {
        if before.epoch != self.epoch || self.working {
            return None;
        }
        self.events
            .iter()
            .filter(|event| {
                event.kind == AttentionKind::Completion && event.sequence > before.sequence
            })
            .map(|event| event.turn)
            .max()
    }
}
