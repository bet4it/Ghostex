//! What the working card's top row shows when it takes two rows: the action's label with any file
//! path shortened to its file name (its folders dimmed and cut to the last one), and how long the
//! current action has run.
//!
//! CDXC:SessionChat 2026-10-28 DECISION:
//! User, of the two-row working bar: "i think there's not enough space between the 2 rows there but think we could do something better also", then picked option C of the mockup (Q724 "divider"): the top row is the working icon and the action, then the action's elapsed time on the right ("14s", muted, tabular digits); a hairline divider in the card's border colour runs across the bar between the rows; the bottom row is the pills, left-aligned, a touch smaller (24px tall, 12px text) with even padding above and below. A file path in the action keeps its file name and dims and shortens its folders ("Reading …/Screenshots/2026-10-28_Ghostex_20-09-21@2x.png"), so the part that matters is not the part cut off. An action with no known start shows no time. The one-row card stays as it was.
//!
//! CDXC:SessionChat 2026-10-28 SEE-ALSO:
//! extras/working_card.rs publishes `labelParts` and `elapsed`; apps/desktop/src/app/native_chat/working_card.rs (desktop and web) and apps/mobile/app/src/chat/native/cards/WorkingCard.tsx (phone) draw them.

use serde::{Deserialize, Serialize};

/// One run of the label: `dim` for a shortened path's folders.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCardLabelPart {
    pub text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dim: bool,
}

/// The current tool action's start, remembered across frames: the transcript's call time when the
/// call is recorded, else the moment its label first showed (a tool read off the agent's screen).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionClock {
    /// What identifies the action: a recorded call's id, or the screen tool's label.
    pub key: String,
    pub started_ms: f64,
}

/// Moves the clock to `action` (its key and recorded start): the same key keeps its start, a new
/// one starts from its recorded time or `now_ms`, and no action clears it.
pub fn settle_action_clock(
    clock: &mut Option<ActionClock>,
    action: Option<(String, Option<i64>)>,
    now_ms: f64,
) {
    match action {
        None => *clock = None,
        Some((key, started)) => {
            if clock.as_ref().is_some_and(|clock| clock.key == key) {
                return;
            }
            *clock = Some(ActionClock {
                key,
                started_ms: started.map_or(now_ms, |started| started as f64),
            });
        }
    }
}

/// How long the action has run, as the top row shows it: "14s", "2m 5s", "2m", "1h 3m"; nothing
/// under a second (a time that would only read "0s").
pub fn action_elapsed_label(elapsed_ms: f64) -> Option<String> {
    if !elapsed_ms.is_finite() || elapsed_ms < 1000.0 {
        return None;
    }
    let seconds = (elapsed_ms / 1000.0).floor() as i64;
    Some(if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        let (minutes, rest) = (seconds / 60, seconds % 60);
        if rest > 0 {
            format!("{minutes}m {rest}s")
        } else {
            format!("{minutes}m")
        }
    } else {
        let (hours, minutes) = (seconds / 3600, seconds % 3600 / 60);
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    })
}

/// Whether the label already counts its own time ("Running 1 shell command · 9s…", "(2m 5s)"), so the
/// top row adds none.
pub fn label_counts_time(label: &str) -> bool {
    label
        .split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '·' | '…' | ','))
        .any(|word| {
            let digits = word.trim_end_matches(['s', 'm', 'h']);
            word.len() > digits.len()
                && !digits.is_empty()
                && digits.chars().all(|c| c.is_ascii_digit())
        })
}

/// The label in runs, each file path in it shortened: its file name stays whole and its folders
/// become one dimmed run, cut to the last folder after "…/" when there are more
/// (`~/Documents/Screenshots/a.png` → "…/Screenshots/" dimmed, then "a.png"). A web address is not a
/// path. A label without a path is one run.
pub fn label_parts(label: &str) -> Vec<WorkingCardLabelPart> {
    let mut parts: Vec<WorkingCardLabelPart> = Vec::new();
    let mut push = |text: &str, dim: bool| {
        if text.is_empty() {
            return;
        }
        match parts.last_mut() {
            Some(last) if last.dim == dim => last.text.push_str(text),
            _ => parts.push(WorkingCardLabelPart {
                text: text.to_string(),
                dim,
            }),
        }
    };
    let mut rest = label;
    while !rest.is_empty() {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let (word, after) = rest.split_at(end);
        let space = after.len() - after.trim_start().len();
        match shorten_path(word) {
            Some((before, folders, file, trailing)) => {
                push(before, false);
                push(&folders, true);
                push(file, false);
                push(trailing, false);
            }
            None => push(word, false),
        }
        push(&after[..space], false);
        rest = &after[space..];
    }
    parts
}

/// A word holding a path, split as (what leads it, its folders shortened, its file name, what
/// trails it): `Read(~/a/b/c.rs)` → ("Read(", "…/b/", "c.rs", ")").
fn shorten_path(word: &str) -> Option<(&str, String, &str, &str)> {
    if word.contains("://") {
        return None;
    }
    let is_edge = |c: char| {
        matches!(
            c,
            '(' | ')' | '"' | '\'' | '`' | ',' | ';' | ':' | '[' | ']'
        )
    };
    // The path starts after the last bracket or quote before its first slash ("Read(~/a.rs").
    let first_slash = word.find('/')?;
    let start = word[..first_slash]
        .char_indices()
        .filter(|(_, c)| is_edge(*c))
        .last()
        .map_or(0, |(index, c)| index + c.len_utf8());
    let end = word
        .char_indices()
        .rev()
        .find(|(_, c)| !is_edge(*c) && *c != '.')
        .map(|(index, c)| index + c.len_utf8())?;
    if end <= start {
        return None;
    }
    let path = &word[start..end];
    let slash = path.rfind('/')?;
    let (folders, file) = (&path[..=slash], &path[slash + 1..]);
    // A folder alone ("src/") or a bare slash is not a file path, and prose ("either/or") is not
    // one either: a path names a file with an extension or starts at a root.
    let rooted = ["/", "~/", "./", "../"]
        .iter()
        .any(|root| path.starts_with(root));
    if file.is_empty() || folders == "/" || !(rooted || file.contains('.')) {
        return None;
    }
    // The folders that name something: a root ("~", ".", "..") is not one.
    let names: Vec<&str> = folders
        .split('/')
        .filter(|name| !matches!(*name, "" | "~" | "." | ".."))
        .collect();
    let shortened = match names.as_slice() {
        // "src/main.rs", "~/notes.md", "~/bin/deploy": short enough as they are.
        [] | [_] => folders.to_string(),
        [.., last] => format!("…/{last}/"),
    };
    Some((&word[..start], shortened, file, &word[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(label: &str) -> Vec<(String, bool)> {
        label_parts(label)
            .into_iter()
            .map(|part| (part.text, part.dim))
            .collect()
    }

    #[test]
    fn a_long_path_keeps_its_file_name_and_dims_its_last_folder() {
        assert_eq!(
            runs("Reading ~/Documents/Screenshots/2026-10-28_Ghostex_20-09-21@2x.png"),
            [
                ("Reading ".to_string(), false),
                ("…/Screenshots/".to_string(), true),
                ("2026-10-28_Ghostex_20-09-21@2x.png".to_string(), false),
            ]
        );
        assert_eq!(
            runs("Read(/Users/me/dev/app/src/main.rs) twice"),
            [
                ("Read(".to_string(), false),
                ("…/src/".to_string(), true),
                ("main.rs) twice".to_string(), false),
            ]
        );
    }

    #[test]
    fn a_short_path_keeps_its_one_folder_dimmed() {
        assert_eq!(
            runs("Editing src/lib.rs."),
            [
                ("Editing ".to_string(), false),
                ("src/".to_string(), true),
                ("lib.rs.".to_string(), false),
            ]
        );
    }

    #[test]
    fn text_without_a_path_is_one_run() {
        assert_eq!(
            runs("Running cargo test"),
            [("Running cargo test".to_string(), false)]
        );
        assert_eq!(
            runs("Fetching https://example.com/a/b.html"),
            [("Fetching https://example.com/a/b.html".to_string(), false)]
        );
        assert_eq!(runs("Listing src/"), [("Listing src/".to_string(), false)]);
        assert_eq!(runs("either/or"), [("either/or".to_string(), false)]);
        assert_eq!(
            runs("Running ~/bin/deploy"),
            [
                ("Running ".to_string(), false),
                ("~/bin/".to_string(), true),
                ("deploy".to_string(), false),
            ]
        );
        assert!(runs("").is_empty());
    }

    #[test]
    fn a_label_that_counts_its_own_time_gets_no_second_one() {
        assert!(label_counts_time("Running 1 shell command · 9s…"));
        assert!(label_counts_time("Bash(cargo test) (2m 5s)"));
        assert!(!label_counts_time("Running 1 shell command"));
        assert!(!label_counts_time(
            "Reading ~/Screenshots/2026-10-28_Ghostex_20-09-21@2x.png"
        ));
        assert!(!label_counts_time("Checking 3 files"));
    }

    #[test]
    fn the_time_shows_from_one_second() {
        assert_eq!(action_elapsed_label(0.0), None);
        assert_eq!(action_elapsed_label(999.0), None);
        assert_eq!(action_elapsed_label(f64::NAN), None);
        assert_eq!(action_elapsed_label(14_200.0).as_deref(), Some("14s"));
        assert_eq!(action_elapsed_label(125_000.0).as_deref(), Some("2m 5s"));
        assert_eq!(action_elapsed_label(120_000.0).as_deref(), Some("2m"));
        assert_eq!(action_elapsed_label(3_780_000.0).as_deref(), Some("1h 3m"));
    }

    #[test]
    fn the_clock_keeps_an_action_and_restarts_for_a_new_one() {
        let mut clock = None;
        settle_action_clock(&mut clock, Some(("Reading a.png".into(), None)), 1_000.0);
        assert_eq!(clock.as_ref().map(|c| c.started_ms), Some(1_000.0));
        settle_action_clock(&mut clock, Some(("Reading a.png".into(), None)), 9_000.0);
        assert_eq!(
            clock.as_ref().map(|c| c.started_ms),
            Some(1_000.0),
            "same action keeps its start"
        );
        settle_action_clock(&mut clock, Some(("call-7".into(), Some(5_000))), 9_000.0);
        assert_eq!(
            clock.as_ref().map(|c| c.started_ms),
            Some(5_000.0),
            "a recorded call keeps its own start"
        );
        settle_action_clock(&mut clock, None, 10_000.0);
        assert_eq!(clock, None);
    }
}
