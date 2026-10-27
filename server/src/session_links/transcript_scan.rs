//! What a session's transcript says about its links: the PRs and issues the agent opened, the
//! GitHub PR and issue URLs the chat only mentioned, and the Linear tickets it mentioned.
//!
//! CDXC:SessionLinks 2026-10-24 DECISION:
//! User picked which links join a session by themselves: the branch's, earlier branches', "PRs and issues the agent opened", what the agent reports with `ghostex link add`, and the issues a linked PR closes. A PR or issue URL the chat only mentioned is offered under Suggested instead ("suggest").
//!
//! CDXC:SessionLinks 2026-10-27 DECISION:
//! User: "pls check why are we linking those SPX linear tickets even tho they're fully unrelated? pls fix". Their 2026-10-24 rule stays ("if the user added linear api key and the linear issue id or link is mentioned in the thread"), and "mentioned in the thread" now means the user's own words: a Linear ID or URL links by itself only from the text of a user row the user wrote. The orchestrator "Mr Big Chungus" (G14iv) had SPX tickets from a thread report and from its own replies ("your SPX-123 session", a dozen times). Then, on agent-written IDs (Q666): "just show them as suggested, dont auto link those". An ID only an agent wrote, in its own reply or in another agent's message or Ghostex thread report sent into the chat (both reach the agent as user rows), is offered under Suggested like a GitHub mention and never linked. Machine text counts for nothing, neither link nor suggestion: harness text (system reminders, hooks, command output, a compaction summary, injected instructions and memory), tool calls and results, a fenced block and a `>` quoted line. A paste still counts as the user's words, since Chat sends every multi-line message as a paste. GitHub URLs follow the same writer rules and stay suggestions from the project's own repos only. The pass keeps only IDs of the Linear teams the project's key sees; Ghostex knows no narrower team for a project. Supersedes linking every ID the user or the agent wrote.
//!
//! CDXC:SessionLinks 2026-10-24 WHY:
//! A transcript is read once in full and then only from where the last read stopped (rows at or past the length it had), so a long session costs a full read once per gxserver run and a page or two per pass after that.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::Value;

use super::{parse_github_url, parse_linear_url, SessionLinkRef};

/// Transcript rows per page, the detail pager's own cap.
const PAGE_ROWS: usize = 200;
/// Pages read at most per transcript on its first read (20,000 rows).
const MAX_PAGES: usize = 100;
/// Transcripts whose findings are kept.
const CACHE_ENTRIES: usize = 512;
/// Mentions kept per transcript for Suggested (GitHub URLs, agent-named Linear IDs), newest last.
const MAX_MENTIONS: usize = 40;

/// What one transcript said, oldest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TranscriptLinks {
    /// PRs and issues a tool call created.
    pub(crate) opened: Vec<SessionLinkRef>,
    /// What the chat only mentioned, newest last: GitHub PR and issue URLs the user or the agent
    /// wrote, and Linear IDs an agent wrote (`own_words`). `suggestions` picks what to offer.
    pub(crate) mentions: Vec<SessionLinkRef>,
    /// Linear IDs (from URLs and bare `GHX-142` words) the user wrote (`own_words`).
    pub(crate) linear_mentions: Vec<String>,
}

/// Whether a Linear ID (`SPX-123`) belongs to one of `team_keys`.
pub(crate) fn of_linear_team(identifier: &str, team_keys: &[String]) -> bool {
    let team = identifier.split('-').next().unwrap_or_default();
    team_keys.iter().any(|key| key.eq_ignore_ascii_case(team))
}

/// Who wrote a piece of the chat.
#[derive(Clone, Copy, PartialEq)]
enum Writer {
    User,
    Agent,
}

impl TranscriptLinks {
    fn push_unique<T: PartialEq>(list: &mut Vec<T>, value: T) {
        if !list.contains(&value) {
            list.push(value);
        }
    }

    /// What to offer under Suggested, newest last: PRs and issues of the project's own repos, and
    /// Linear tickets of the teams its Linear key sees that only an agent named (one the user
    /// wrote links instead).
    pub(crate) fn suggestions(
        &self,
        own_repos: &[String],
        team_keys: &[String],
    ) -> Vec<SessionLinkRef> {
        self.mentions
            .iter()
            .filter(|link| match link {
                SessionLinkRef::Linear { identifier } => {
                    of_linear_team(identifier, team_keys)
                        && !self.linear_mentions.contains(identifier)
                }
                SessionLinkRef::Github { .. } => link.in_repos(own_repos),
            })
            .cloned()
            .collect()
    }

    fn mention_github(&mut self, link: SessionLinkRef) {
        Self::push_newest(&mut self.mentions, link);
    }

    fn mention_linear(&mut self, identifier: String, writer: Writer) {
        match writer {
            Writer::User => Self::push_unique(&mut self.linear_mentions, identifier),
            Writer::Agent => {
                Self::push_newest(&mut self.mentions, SessionLinkRef::Linear { identifier })
            }
        }
    }

    /// Moves `value` to the end of a list kept to its newest `MAX_MENTIONS`.
    fn push_newest<T: PartialEq>(list: &mut Vec<T>, value: T) {
        list.retain(|known| known != &value);
        list.push(value);
        if list.len() > MAX_MENTIONS {
            list.remove(0);
        }
    }
}

/// The scan's state for one transcript, so the next pass continues where this one stopped.
#[derive(Default)]
struct Scanner {
    links: TranscriptLinks,
    /// Tool calls that create a PR or issue, waiting for their result.
    creating_calls: HashSet<String>,
    /// The last tool call created something and recorded no call id (agents that pair a result
    /// with the call right before it).
    creating_without_id: bool,
}

/// The tool input of a call that creates a PR or an issue: `gh pr create`, `gh issue create`, or a
/// GitHub tool server's `create_pull_request` / `create_issue`.
fn creates_github_item(name: &str, input: &Value) -> bool {
    let input = match input {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let name = name.to_ascii_lowercase();
    input.contains("gh pr create")
        || input.contains("gh issue create")
        || name.contains("create_pull_request")
        || name.contains("create_issue")
}

impl Scanner {
    fn scan_message(&mut self, message: &Value) {
        let role = crate::ghostex_cli::session_chat_transcript::effective_role(message);
        // A whole row another agent or Ghostex sent (its sender block may sit in its own block):
        // its body is the agent's words.
        let agent_row = (role == "user")
            .then(|| {
                agent_message_body(&crate::ghostex_cli::session_chat_transcript::message_text(
                    message,
                    usize::MAX,
                ))
            })
            .flatten();
        if let Some(body) = &agent_row {
            self.scan_text(&own_words(body), Writer::Agent);
        }
        let blocks = message
            .get("blocks")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for block in blocks {
            match block.get("type").and_then(Value::as_str) {
                Some("tool-call") => {
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let creates =
                        creates_github_item(name, block.get("input").unwrap_or(&Value::Null));
                    match block.get("callId").and_then(Value::as_str) {
                        Some(call_id) if creates => {
                            self.creating_calls.insert(call_id.to_string());
                        }
                        Some(_) => {}
                        None => self.creating_without_id = creates,
                    }
                }
                Some("tool-result") => {
                    let created = match block.get("callId").and_then(Value::as_str) {
                        Some(call_id) => self.creating_calls.remove(call_id),
                        None => std::mem::take(&mut self.creating_without_id),
                    };
                    let failed = block.get("isError").and_then(Value::as_bool) == Some(true);
                    if created && !failed {
                        let output = block.get("output").and_then(Value::as_str).unwrap_or("");
                        for link in urls_in(output, "https://github.com/")
                            .filter_map(|url| parse_github_url(&url))
                        {
                            TranscriptLinks::push_unique(&mut self.links.opened, link);
                        }
                    }
                }
                Some("text") if role == "user" && agent_row.is_none() => {
                    let text = block.get("text").and_then(Value::as_str).unwrap_or("");
                    if let Some(body) = agent_message_body(text) {
                        self.scan_text(&own_words(&body), Writer::Agent);
                    } else if !is_harness_text(text) {
                        self.scan_text(&own_words(text), Writer::User);
                    }
                }
                Some("text") if role == "assistant" => {
                    let text = block.get("text").and_then(Value::as_str).unwrap_or("");
                    self.scan_text(&own_words(text), Writer::Agent);
                }
                _ => {}
            }
        }
    }

    /// Reads the URLs and IDs in `text`, which `writer` wrote.
    fn scan_text(&mut self, text: &str, writer: Writer) {
        for link in urls_in(text, "https://github.com/").filter_map(|url| parse_github_url(&url)) {
            self.links.mention_github(link);
        }
        for link in urls_in(text, "https://linear.app/").filter_map(|url| parse_linear_url(&url)) {
            if let SessionLinkRef::Linear { identifier } = link {
                self.links.mention_linear(identifier, writer);
            }
        }
        for identifier in bare_linear_identifiers(text) {
            self.links.mention_linear(identifier, writer);
        }
    }
}

/// The body of another agent's message on a user row (its sender block above or below it, or a
/// `<cross-session-message>` envelope) or of a Ghostex thread report: an agent's words.
fn agent_message_body(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some(message) =
        ghostex_gx_chat_core::transcript::agent_message::parse_inter_agent_message(text)
    {
        return Some(message.body);
    }
    text.starts_with("Ghostex thread report:")
        .then(|| text.to_string())
}

/// What the agent CLI or Ghostex injects on a user row (system reminders, hook output, a
/// compaction summary, AGENTS.md and environment context): nobody's words.
fn is_harness_text(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && (crate::session_chat_decode_claude::is_known_harness_injected_user_turn_text(text)
            || crate::agents::activity::is_first_prompt_claim_meta_prompt(text))
}

/// `text` without what it quotes: harness tag spans (`<system-reminder>…</system-reminder>`),
/// fenced blocks (logs, code, command output) and `>` quoted lines.
fn own_words(text: &str) -> String {
    let mut text = text.to_string();
    for tag in crate::session_chat_decode_claude::KNOWN_HARNESS_TAG_NAMES {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        while let Some(start) = text.find(&open) {
            let end = text[start..]
                .find(&close)
                .map_or(text.len(), |at| start + at + close.len());
            text.replace_range(start..end, " ");
        }
    }
    let mut kept = String::with_capacity(text.len());
    let mut fence: Option<&str> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"]
            .into_iter()
            .find(|marker| trimmed.starts_with(marker));
        match (fence, marker) {
            (None, Some(marker)) => fence = Some(marker),
            (Some(open), Some(marker)) if open == marker => fence = None,
            (None, None) if !trimmed.starts_with('>') => {
                kept.push_str(line);
                kept.push('\n');
            }
            _ => {}
        }
    }
    kept
}

/// Every address in `text` that starts with `prefix`, cut where the address ends.
fn urls_in<'a>(text: &'a str, prefix: &'a str) -> impl Iterator<Item = String> + 'a {
    text.match_indices(prefix).map(move |(start, _)| {
        text[start..]
            .chars()
            .take_while(|c| {
                !c.is_whitespace() && !matches!(c, ')' | ']' | '>' | '<' | '"' | '\'' | '`' | ',')
            })
            .collect::<String>()
            .trim_end_matches(['.', ':', ';'])
            .to_string()
    })
}

/// `GHX-142`-shaped words: an uppercase key that starts with a letter, a dash, digits. Lowercase
/// spellings are left out (`utf-8`, a branch's `spx-12`); the pass keeps only real team keys.
fn bare_linear_identifiers(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter_map(|word| {
            let (key, number) = word.split_once('-')?;
            let valid = (1..=10).contains(&key.len())
                && key.starts_with(|c: char| c.is_ascii_uppercase())
                && key
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                && (1..=7).contains(&number.len())
                && number.chars().all(|c| c.is_ascii_digit());
            valid.then(|| format!("{key}-{number}"))
        })
}

struct CachedScan {
    len: u64,
    modified: Option<SystemTime>,
    scanner: Scanner,
}

fn cache() -> &'static Mutex<HashMap<PathBuf, CachedScan>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedScan>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Whether reading this transcript now would read it in full (it was never read, or it shrank),
/// which the pass rations.
pub(crate) fn needs_full_scan(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(path).map(|cached| metadata.len() < cached.len))
        .unwrap_or(true)
}

/// What the transcript said so far, reading only what was added since the last call.
pub(crate) fn scan_transcript(
    agent: crate::session_chat::SessionChatTranscriptAgent,
    path: &Path,
    project_id: &str,
    session_id: &str,
) -> Option<TranscriptLinks> {
    let metadata = std::fs::metadata(path).ok()?;
    let (len, modified) = (metadata.len(), metadata.modified().ok());
    let mut previous = cache().lock().ok()?.remove(path);
    if let Some(cached) = &previous {
        if cached.len == len && cached.modified == modified {
            let links = cached.scanner.links.clone();
            if let Ok(mut cache) = cache().lock() {
                cache.insert(path.to_path_buf(), previous.take()?);
            }
            return Some(links);
        }
    }
    // A shrunk file was rewritten: read it again from the start.
    let (mut scanner, from) = match previous {
        Some(cached) if cached.len <= len => (cached.scanner, cached.len),
        _ => (Scanner::default(), 0),
    };
    for message in read_rows_since(agent, path, len, from, project_id, session_id) {
        scanner.scan_message(&message);
    }
    let links = scanner.links.clone();
    if let Ok(mut cache) = cache().lock() {
        if cache.len() >= CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(
            path.to_path_buf(),
            CachedScan {
                len,
                modified,
                scanner,
            },
        );
    }
    Some(links)
}

/// The rows at or past byte `from`, oldest first.
fn read_rows_since(
    agent: crate::session_chat::SessionChatTranscriptAgent,
    path: &Path,
    len: u64,
    from: u64,
    project_id: &str,
    session_id: &str,
) -> Vec<Value> {
    let mut before = len;
    let mut rows: Vec<Value> = Vec::new();
    for _ in 0..MAX_PAGES {
        let Ok(page) = crate::session_chat_history::read_history(
            agent, path, before, PAGE_ROWS, true, false, project_id, session_id,
        ) else {
            break;
        };
        let mut older: Vec<Value> = page
            .get("messages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let next = page
            .get("beforeOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let reached_old_rows = next <= from;
        if from > 0 {
            older.retain(|row| {
                row.get("byteOffset")
                    .and_then(Value::as_u64)
                    .is_none_or(|offset| offset >= from)
            });
        }
        older.extend(rows);
        rows = older;
        if reached_old_rows
            || page.get("hasMore").and_then(Value::as_bool) != Some(true)
            || next >= before
        {
            break;
        }
        before = next;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the links pass offers from a transcript (session_links_sync.rs): only mentions in the
    /// project's own repos.
    fn suggested(text: &str, own_repos: &[String]) -> Vec<String> {
        let mut scanner = Scanner::default();
        scanner.scan_text(text, Writer::Agent);
        scanner
            .links
            .suggestions(own_repos, &[])
            .into_iter()
            .map(|link| link.key())
            .collect()
    }

    #[test]
    fn a_foreign_repo_url_in_the_transcript_is_not_suggested() {
        let text = "Read https://github.com/joeblau/voice/pull/118 for the turn orchestrator, \
                    then fix https://github.com/maddada/Ghostex/issues/283 here.";
        let own = vec!["maddada/ghostex".to_string()];
        assert_eq!(suggested(text, &own), vec!["issue:maddada/ghostex#283"]);
        // The fork's upstream counts as the project's own repo.
        let fork = vec!["me/ghostex".to_string(), "maddada/ghostex".to_string()];
        assert_eq!(suggested(text, &fork), vec!["issue:maddada/ghostex#283"]);
        // A project with no GitHub remote is offered nothing from GitHub.
        assert!(suggested(text, &[]).is_empty());
        // Linear tickets keep working as before.
        assert!(SessionLinkRef::Linear {
            identifier: "GHX-7".to_string()
        }
        .in_repos(&[]));
    }

    fn claude_row(kind: &str, content: Value, extra: Value) -> String {
        let mut row = serde_json::json!({
            "type": kind,
            "uuid": format!("{kind}-{}", content.to_string().len()),
            "timestamp": "2026-10-27T10:00:00.000Z",
            "message": { "role": kind, "content": content },
        });
        if let (Some(row), Some(extra)) = (row.as_object_mut(), extra.as_object()) {
            row.extend(extra.clone());
        }
        row.to_string()
    }

    /// What a Claude transcript links and suggests (Linear), read the way the links pass reads it.
    fn linear_mentions_of(rows: &[String]) -> (Vec<String>, Vec<String>) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("transcript.jsonl");
        std::fs::write(&path, rows.join("\n") + "\n").expect("write transcript");
        let links = scan_transcript(
            crate::session_chat::SessionChatTranscriptAgent::Claude,
            &path,
            "P1",
            "S1",
        )
        .expect("scan");
        let sorted = |mut list: Vec<String>| {
            list.sort();
            list
        };
        let suggested = links
            .suggestions(&[], &["SPX".to_string()])
            .iter()
            .filter_map(|link| match link {
                SessionLinkRef::Linear { identifier } => Some(identifier.clone()),
                SessionLinkRef::Github { .. } => None,
            })
            .collect();
        (sorted(links.linear_mentions), sorted(suggested))
    }

    /// CDXC:SessionLinks 2026-10-27: only the user's own words link a Linear ticket; an agent's
    /// words suggest it; machine text does neither.
    #[test]
    fn only_the_users_own_words_link_a_linear_ticket() {
        let none = Value::Null;
        let rows = vec![
            // Another agent's report, pasted in by Ghostex with its sender block below it.
            claude_row(
                "user",
                Value::String(
                    "<pasted_content id=\"ab12\">\nGhostex thread report: finished its turn. Its final message:\n\n\
                     - **Linear chips:** the user's sleeping ShortPoint sessions show them (SPX-13211, SPX-12965, SPX-12599).\n\n\
                     Message from another agent\nAgent: claude\nSession: Linear chips\nSession ID: G6o7z\n\
                     Agent ID: claude\nAgent Session ID: abc\nReply to: S90:P3lv0:G6o7z\n</pasted_content id=\"ab12\">"
                        .to_string(),
                ),
                none.clone(),
            ),
            // A tool call and its result that name a ticket (a session list, a file).
            claude_row(
                "assistant",
                serde_json::json!([{ "type": "tool_use", "id": "t1", "name": "Bash",
                    "input": { "command": "ghostex list-sessions | grep SPX-901" } }]),
                none.clone(),
            ),
            claude_row(
                "user",
                serde_json::json!([{ "type": "tool_result", "tool_use_id": "t1",
                    "content": "S90:P3lv0:G6o7z\tClaude (cloud)\tidle\tGhostex\tSPX-902" }]),
                none.clone(),
            ),
            // The agent's own reply repeating what it read.
            claude_row(
                "assistant",
                serde_json::json!([{ "type": "text", "text": "Your SPX-903 session started fine." }]),
                none.clone(),
            ),
            // The user's own words, with a reminder, a log and a quote around them.
            claude_row(
                "user",
                serde_json::json!([
                    { "type": "text", "text": "work on SPX-123 and https://linear.app/acme/issue/SPX-124/title\n\
                        > the report said SPX-904\n```\nERROR in SPX-905 build\n```" },
                    { "type": "text", "text": "<system-reminder>The task list mentions SPX-906.</system-reminder>" },
                ]),
                none.clone(),
            ),
            // Claude's summary of the conversation after a compaction.
            claude_row(
                "user",
                Value::String(
                    "This session is being continued from a previous conversation. Your SPX-907 session...".to_string(),
                ),
                serde_json::json!({ "isCompactSummary": true }),
            ),
        ];
        let (linked, suggested) = linear_mentions_of(&rows);
        assert_eq!(linked, vec!["SPX-123", "SPX-124"]);
        // The report's body and the agent's reply; not its tool call, the tool's output, the
        // quoted line, the log, the reminder or the compaction summary (SPX-901, 902, 904-907).
        assert_eq!(
            suggested,
            vec!["SPX-12599", "SPX-12965", "SPX-13211", "SPX-903"]
        );
    }

    #[test]
    fn a_ticket_only_an_agent_named_is_suggested_not_linked() {
        let mut scanner = Scanner::default();
        scanner.scan_text(
            "Your SPX-123 session and GHX-9 are ready; also OTHER-4.",
            Writer::Agent,
        );
        scanner.scan_text("ok, work on GHX-9", Writer::User);
        let teams = vec!["SPX".to_string(), "GHX".to_string()];
        let keys = |links: Vec<SessionLinkRef>| -> Vec<String> {
            links.iter().map(SessionLinkRef::key).collect()
        };
        // GHX-9 the user wrote too, so it links instead; OTHER is not a team of the key.
        assert_eq!(scanner.links.linear_mentions, vec!["GHX-9"]);
        assert_eq!(
            keys(scanner.links.suggestions(&[], &teams)),
            vec!["linear:SPX-123"]
        );
    }

    #[test]
    fn own_words_drops_quotes_fences_and_harness_spans() {
        let text = "keep GHX-1\n> quoted GHX-2\n~~~\nGHX-3\n~~~\nkeep GHX-4 <system-reminder>GHX-5</system-reminder> after";
        let words = own_words(text);
        assert!(words.contains("GHX-1") && words.contains("GHX-4") && words.contains("after"));
        for dropped in ["GHX-2", "GHX-3", "GHX-5"] {
            assert!(!words.contains(dropped), "{dropped} in {words:?}");
        }
        // An agent's message with its sender block first (transcripts before 2026-10-05).
        assert_eq!(
            agent_message_body(
                "Message from another agent\nAgent: codex\nReply to: S1:P1:S2\n\nSee SPX-1."
            )
            .as_deref(),
            Some("See SPX-1.")
        );
        assert!(agent_message_body("Please look at SPX-1 for me").is_none());
        assert!(is_harness_text("<system-reminder>SPX-1</system-reminder>"));
    }
}
