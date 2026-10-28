//! A message held for an idle Claude session whose input box cannot be read: ask Claude to redraw.
//!
//! CDXC:SessionChat 2026-10-28 WHY:
//! A coordinator's thread reports sat as "Message from … · Waiting for agent" for fifteen minutes while the coordinator was idle (S90:P3lv0:G1pif, 2:50 to 3:05 PM). gxserver held the first report because "The claude input box is not on screen yet." and queued the next four behind it as startup sends. The scheduler re-read the screen every few seconds, through a whole turn the coordinator ran in between, and it never read as an input box. It went through only when the user opened the session, which resized and repainted Claude. The send path already answers such a screen with Claude's Ctrl+L redraw (`composer_repaint.rs`), but a held message never reaches the send path, and an idle, unwatched Claude never repaints by itself. So the scheduler asks for the same redraw. Only for a running Claude session: not mid-turn and not compacting; no notice, dialog or panel on screen; Claude's `❯` prompt row visible; the agent not still starting; and the input box unreadable for 20 s. Then at most once a minute, and every redraw is logged with the screen it saw (`sessionChatQueueRedrewClaude` in session-chat-send-failures.jsonl), so a frame that still does not parse can be read and fixed in the detector.
//! SEE-ALSO: session_chat_send/composer_repaint.rs (the send path's redraw), scheduler.rs (the startup-send hold that calls the refresher), wiring.rs (`session_chat_queue_compacting_refresher`, which runs this after each fresh screen read).

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::agents::identity::normalize_agent_id;
use crate::server::AppState;
use crate::session_chat_options::SessionChatTerminalDetection;

/// How long the input box must stay unreadable before the first redraw: a CLI mid-paint or still
/// booting settles well within it.
const FIRST_REDRAW_AFTER: Duration = Duration::from_secs(20);
/// The least time between two redraws of one session.
const REDRAW_EVERY: Duration = Duration::from_secs(60);

/// Claude's `Ctrl+L`, which repaints its whole screen and keeps the conversation and any typed text.
const CLAUDE_REDRAW: &str = "\u{c}";

#[derive(Clone, Copy)]
struct Clock {
    unreadable_since: Instant,
    last_redraw: Option<Instant>,
}

static CLOCKS: LazyLock<Mutex<HashMap<String, Clock>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether this fresh screen of a held session is one a redraw may fix: Claude, its input box
/// missing with nothing else in its place, its prompt row on screen, and not a CLI still starting.
pub(crate) fn screen_wants_redraw(
    agent: Option<&str>,
    detection: &SessionChatTerminalDetection,
) -> bool {
    let composer = &detection.composer;
    normalize_agent_id(agent).as_deref() == Some("claude")
        && detection.notice.is_none()
        && detection.activity.is_none()
        && composer.is_not_ready()
        && composer.input_box_missing()
        && !composer.should_dismiss()
        && composer
            .screen_tail
            .iter()
            .any(|line| line.trim_start().starts_with('\u{276f}'))
        && !crate::session_chat_send_wake::screen_shows_agent_starting(&composer.screen_tail)
}

/// Whether a redraw is due for a session whose input box has been unreadable since
/// `unreadable_since` and was last redrawn at `last_redraw`.
pub(crate) fn redraw_due(
    unreadable_since: Instant,
    last_redraw: Option<Instant>,
    now: Instant,
) -> bool {
    now.duration_since(unreadable_since) >= FIRST_REDRAW_AFTER
        && last_redraw.is_none_or(|last| now.duration_since(last) >= REDRAW_EVERY)
}

/// The session may take a message now: running, not mid-turn by its hooks, not compacting.
fn session_is_idle(session: &Value) -> bool {
    let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    session.get("lifecycleState").and_then(Value::as_str) == Some("running")
        && crate::session_chat_compacting::session_chat_compacting_detected_at(session).is_none()
        && super::scheduler::session_chat_queue_activity_is_deliverable(session, &generated_at)
}

/// Runs after the scheduler's fresh screen read of a session holding a message: redraws Claude
/// when its unreadable input box has outlasted [`FIRST_REDRAW_AFTER`], at most every
/// [`REDRAW_EVERY`]. Blocking: it runs on the refresher's own blocking task.
pub(crate) fn redraw_held_claude_if_due(
    state: &AppState,
    project_id: &str,
    session_id: &str,
    agent: Option<&str>,
    detection: &SessionChatTerminalDetection,
) {
    let key = crate::server::session_observer_key(project_id, session_id);
    let now = Instant::now();
    let Ok(mut clocks) = CLOCKS.lock() else {
        return;
    };
    if !screen_wants_redraw(agent, detection) {
        clocks.remove(&key);
        return;
    }
    let clock = *clocks.entry(key.clone()).or_insert(Clock {
        unreadable_since: now,
        last_redraw: None,
    });
    if !redraw_due(clock.unreadable_since, clock.last_redraw, now) {
        return;
    }
    let Some(session) = read_session(state, project_id, session_id) else {
        return;
    };
    if !session_is_idle(&session) {
        return;
    }
    let Ok(zmx_name) = crate::zmx::provider_zmx_session_name(&session) else {
        return;
    };
    clocks.insert(
        key,
        Clock {
            last_redraw: Some(now),
            ..clock
        },
    );
    drop(clocks);
    crate::session_chat_send_diagnostics::record_send_recovery(
        state,
        "sessionChatQueueRedrewClaude",
        project_id,
        session_id,
        detection.composer.reason.as_deref().unwrap_or_default(),
        &detection.composer.screen_tail,
    );
    crate::zmx::log_temporary_zmx_input_write(
        project_id,
        session_id,
        &zmx_name,
        "sessionChatQueueRedraw",
        "session-chat-message",
        CLAUDE_REDRAW,
    );
    let _ = crate::zmx::session_chat_zmx_write(&zmx_name, CLAUDE_REDRAW);
}

fn read_session(state: &AppState, project_id: &str, session_id: &str) -> Option<Value> {
    let db = crate::storage::open_gxserver_database(&state.paths).ok()?;
    let repository = crate::domain::DomainRepository::new(&db, state.metadata.server_id.as_str());
    repository
        .get_session(project_id, session_id)
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_chat_composer::detect_session_chat_composer_readiness;

    fn detection(screen: &str) -> SessionChatTerminalDetection {
        SessionChatTerminalDetection {
            composer: detect_session_chat_composer_readiness(Some("claude"), screen, None),
            ..SessionChatTerminalDetection::default()
        }
    }

    const RULE: &str =
        "────────────────────────────────────────────────────────────────────────────";

    /// The idle coordinator's prompt row with its box only half drawn (the bottom rule gone), as a
    /// capture caught it: no input box reads, Claude's `❯` is there.
    fn half_drawn() -> String {
        format!("⏺ Done.\n\n  22 tasks (20 done, 2 open)\n{RULE} Mr Big Chungus ─\n❯ \n")
    }

    #[test]
    fn an_idle_claude_screen_without_a_readable_box_is_redrawn() {
        let screen = half_drawn();
        let seen = detection(&screen);
        assert!(seen.composer.input_box_missing(), "{:?}", seen.composer);
        assert!(screen_wants_redraw(Some("claude"), &seen));
        // The coordinator runs as a custom agent whose composer is Claude's.
        assert!(!screen_wants_redraw(Some("codex"), &seen));
    }

    #[test]
    fn a_readable_box_a_starting_agent_or_no_prompt_row_is_left_alone() {
        let whole = format!("⏺ Done.\n{RULE} Mr Big Chungus ─\n❯ \n{RULE}\n");
        assert!(!screen_wants_redraw(Some("claude"), &detection(&whole)));
        let starting = "Restoring session...\n";
        assert!(!screen_wants_redraw(Some("claude"), &detection(starting)));
        let no_prompt = format!("⏺ Done.\n{RULE}\n");
        assert!(!screen_wants_redraw(Some("claude"), &detection(&no_prompt)));
    }

    #[test]
    fn a_redraw_waits_twenty_seconds_then_comes_at_most_once_a_minute() {
        let since = Instant::now();
        assert!(!redraw_due(since, None, since + Duration::from_secs(19)));
        assert!(redraw_due(since, None, since + Duration::from_secs(20)));
        let last = since + Duration::from_secs(20);
        assert!(!redraw_due(
            since,
            Some(last),
            last + Duration::from_secs(59)
        ));
        assert!(redraw_due(
            since,
            Some(last),
            last + Duration::from_secs(60)
        ));
    }
}
