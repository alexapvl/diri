//! Finished turns, derived from the Engine's `session.updated` stream the
//! same way the Mac app's voice reader derives them, so the phone hears about
//! exactly the turns the Mac would read aloud.

use std::collections::HashMap;

use diri_proto::attention::AttentionState;
use diri_proto::{EventName, SessionRecord};
use serde::Deserialize;
use serde_json::Value;

/// The SSE event a phone voice client listens for.
pub const TURN_FINISHED: &str = "turn_finished";

/// Only the fields a completion needs; the rest of the record is skipped
/// rather than parsed on every update.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attention {
    id: String,
    #[serde(default)]
    attention_state: Option<AttentionState>,
}

/// Each session's last seen attention state, per event stream.
#[derive(Default)]
pub struct TurnWatcher {
    last: HashMap<String, AttentionState>,
}

impl TurnWatcher {
    /// Starts from what the sessions look like now. A session first seen in
    /// an update only sets its baseline: a reconnect never replays every
    /// session's last reply as just finished.
    pub fn seed(&mut self, states: impl IntoIterator<Item = (String, AttentionState)>) {
        self.last = states.into_iter().collect();
    }

    /// [`Self::seed`] from the Engine's session list.
    pub fn seed_records(&mut self, records: Vec<SessionRecord>) {
        self.seed(records.into_iter().filter_map(|record| {
            let state = record.attention_state?;
            Some((record.id.0, state))
        }));
    }

    /// The `(session, turn)` this event finished, if any.
    pub fn observe(&mut self, name: &str, params: &Value) -> Option<(String, u64)> {
        match name {
            EventName::SESSION_UPDATED => {
                let update = Attention::deserialize(params).ok()?;
                let current = update.attention_state?;
                let finished = self
                    .last
                    .get(&update.id)
                    .and_then(|before| current.completed_turn_since(before));
                self.last.insert(update.id.clone(), current);
                finished.map(|turn| (update.id, turn))
            }
            EventName::SESSION_REMOVED => {
                if let Some(id) = params.get("id").and_then(Value::as_str) {
                    self.last.remove(id);
                }
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diri_proto::attention::{AttentionEvent, AttentionKind};
    use serde_json::json;

    fn state(epoch: &str, sequence: u64, working: bool, completions: &[(u64, u64)]) -> Value {
        let events: Vec<AttentionEvent> = completions
            .iter()
            .map(|&(sequence, turn)| AttentionEvent {
                sequence,
                turn,
                kind: AttentionKind::Completion,
                occurred_at: diri_proto::DateMillis(0.0),
                resolved: false,
                blocking: false,
                detail: None,
            })
            .collect();
        serde_json::to_value(AttentionState {
            version: diri_proto::attention::ATTENTION_VERSION,
            epoch: epoch.into(),
            sequence,
            turn: completions.last().map_or(0, |&(_, turn)| turn),
            working,
            observed_at: None,
            last_native_completion: None,
            active_tools: Default::default(),
            events,
            native_requests: Default::default(),
            native_completions: Default::default(),
        })
        .unwrap()
    }

    fn update(id: &str, attention: Value) -> Value {
        json!({"id": id, "title": "ignored", "attentionState": attention})
    }

    #[test]
    fn a_completion_after_the_baseline_is_a_finished_turn_once() {
        let mut watcher = TurnWatcher::default();
        let name = EventName::SESSION_UPDATED;
        // First sight only sets the baseline.
        assert_eq!(
            watcher.observe(name, &update("s_a", state("e", 2, true, &[]))),
            None
        );
        assert_eq!(
            watcher.observe(name, &update("s_a", state("e", 3, false, &[(3, 1)]))),
            Some(("s_a".into(), 1))
        );
        // The same state again is not a second completion.
        assert_eq!(
            watcher.observe(name, &update("s_a", state("e", 3, false, &[(3, 1)]))),
            None
        );
        // Still working, or a restarted history, is not a completion.
        assert_eq!(
            watcher.observe(name, &update("s_a", state("e", 5, true, &[(3, 1), (5, 2)]))),
            None
        );
        assert_eq!(
            watcher.observe(name, &update("s_a", state("f", 1, false, &[(1, 1)]))),
            None
        );
        // A removed session starts over.
        watcher.observe(EventName::SESSION_REMOVED, &json!({"id": "s_a"}));
        assert_eq!(
            watcher.observe(
                name,
                &update("s_a", state("f", 2, false, &[(1, 1), (2, 2)]))
            ),
            None
        );
        // Unrelated or malformed events are ignored.
        assert_eq!(watcher.observe("workspace.updated", &json!({})), None);
        assert_eq!(watcher.observe(name, &json!({"nope": true})), None);
    }

    #[test]
    fn a_seeded_session_reports_its_next_completion() {
        let mut watcher = TurnWatcher::default();
        let seeded: AttentionState = serde_json::from_value(state("e", 4, true, &[])).unwrap();
        watcher.seed([("s_b".to_string(), seeded)]);
        assert_eq!(
            watcher.observe(
                EventName::SESSION_UPDATED,
                &update("s_b", state("e", 6, false, &[(6, 7)]))
            ),
            Some(("s_b".into(), 7))
        );
    }
}
