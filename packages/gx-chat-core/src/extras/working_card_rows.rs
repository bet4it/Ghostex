//! How many rows the working card above the composer takes: one (the label and the pills side by
//! side) or two (the label on its own row on top, the pills under it). The renderers measure, this
//! decides, so the desktop, the web build and the phone (through the `workingCardRows` query) agree.
//!
//! CDXC:SessionChat 2026-10-28 DECISION:
//! User, with a phone screenshot of the bar above the composer: "I cannot read the actual task the model did. I think we explained that it should be 2 rows high for the bar above the composer if there is not enough space to fit the text on the left for the working indicator", "The bottom row should be the pills, which I can click on. The top row needs to be the current action, or whatever you call it, or the working text, with the spinner there", "I don't want the bar above the composer to keep going between 1 row and 2 rows over and over. It should sort of be stable", and "It should animate when it expands to show 2 rows and collapses to show just 1 row". So the card takes two rows when the label and the pills don't fit one line, the top row the lead glyph and the whole label's line, the bottom row the pills; the rule below keeps it from flapping. Supersedes the 2026-10-17 one line with the label cut to make room for the pills (extras/working_card.rs); the top row still cuts a label longer than the whole card with an ellipsis, whose full text shows on hover (a long press on the phone).
//!
//! The rule (`next`):
//! - Two rows only when there is something to split: a label and at least one pill. Otherwise one.
//! - Expand as soon as the label and the pills, with `EXPAND_MARGIN` to spare, don't fit one line.
//! - While a turn runs (`phase` "working"), two rows stay two rows: a turn's labels change with every
//!   tool, and a short label after a long one must not pull the pills back up. So a turn expands at
//!   most once.
//! - Out of a turn, collapse only when everything fits with `COLLAPSE_MARGIN` to spare (wider than a
//!   pill, so the scroll-to-bottom or ? pill coming and going never flips it) and two rows have
//!   shown for at least `MIN_TWO_ROWS_MS` (a turn that ends right after it expanded doesn't bounce).
//!   When the hold is all that keeps it at two rows, `recheck_at_ms` says when to ask again.
//!
//! CDXC:SessionChat 2026-10-28 SEE-ALSO:
//! apps/desktop/src/app/native_chat/working_card.rs (desktop and web) and apps/mobile/app/src/chat/native/cards/WorkingCard.tsx (phone) measure, ask and animate the change in `ANIMATION_MS`.

use serde::{Deserialize, Serialize};

/// Room (px) the one-row line must have left over to stay one row.
pub const EXPAND_MARGIN: f32 = 8.0;
/// Room (px) everything must leave over before two rows go back to one.
pub const COLLAPSE_MARGIN: f32 = 48.0;
/// The shortest time two rows show before they may go back to one.
pub const MIN_TWO_ROWS_MS: f64 = 2_000.0;
/// How long the card takes to grow into two rows or shrink back (none with reduce motion on).
pub const ANIMATION_MS: u64 = 200;

/// What the renderer measured, in its own units (px or dp), all at natural width.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RowsInput {
    /// The card's inner width, between its paddings.
    pub width: f32,
    /// The lead glyph and the gap after it, or 0 without one.
    pub lead: f32,
    /// The label on one line, uncut, with whatever trails it (a compaction's numbers); 0 without one.
    pub label: f32,
    /// Every pill and timer side by side, with their gaps; 0 without any.
    pub pills: f32,
    /// The gap between the label and the pills on one line.
    pub gap: f32,
    /// The card's `phase`: "working", "worked" or "idle".
    pub phase: String,
    /// The clock, in ms.
    pub now_ms: f64,
}

/// The decision, which the renderer keeps and passes back with its next measurement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RowsMemo {
    /// 1 or 2 (0 before the first measurement reads as 1).
    pub rows: u8,
    /// When the card went to two rows.
    pub since_ms: f64,
    /// Ask again at this time even if nothing changed: only the minimum hold keeps two rows.
    pub recheck_at_ms: Option<f64>,
}

impl RowsMemo {
    pub fn two_rows(&self) -> bool {
        self.rows == 2
    }
}

/// The next decision from the last one and a fresh measurement.
pub fn next(memo: &RowsMemo, input: &RowsInput) -> RowsMemo {
    let one = RowsMemo {
        rows: 1,
        since_ms: 0.0,
        recheck_at_ms: None,
    };
    if input.label <= 0.0 || input.pills <= 0.0 || input.width <= 0.0 {
        return one;
    }
    let line = input.lead + input.label + input.gap + input.pills;
    if !memo.two_rows() {
        return if line + EXPAND_MARGIN > input.width {
            RowsMemo {
                rows: 2,
                since_ms: input.now_ms,
                recheck_at_ms: None,
            }
        } else {
            one
        };
    }
    let stay = RowsMemo {
        recheck_at_ms: None,
        ..*memo
    };
    if input.phase == "working" || line + COLLAPSE_MARGIN > input.width {
        return stay;
    }
    let hold_until = memo.since_ms + MIN_TWO_ROWS_MS;
    if input.now_ms < hold_until {
        return RowsMemo {
            recheck_at_ms: Some(hold_until),
            ..stay
        };
    }
    one
}

/// The `workingCardRows(memo, input)` query: the memo may be `null` before the first measurement.
pub fn query(arguments: &[serde_json::Value]) -> Option<RowsMemo> {
    let memo = match arguments.first() {
        None | Some(serde_json::Value::Null) => RowsMemo::default(),
        Some(memo) => serde_json::from_value(memo.clone()).ok()?,
    };
    let input: RowsInput = serde_json::from_value(arguments.get(1)?.clone()).ok()?;
    Some(next(&memo, &input))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(width: f32, label: f32, pills: f32, phase: &str, now_ms: f64) -> RowsInput {
        RowsInput {
            width,
            lead: 24.0,
            label,
            pills,
            gap: 8.0,
            phase: phase.to_string(),
            now_ms,
        }
    }

    #[test]
    fn one_row_while_everything_fits() {
        let memo = next(
            &RowsMemo::default(),
            &input(400.0, 120.0, 150.0, "working", 0.0),
        );
        assert_eq!(memo.rows, 1);
    }

    #[test]
    fn two_rows_when_the_label_and_the_pills_overflow_one_line() {
        let memo = next(
            &RowsMemo::default(),
            &input(300.0, 180.0, 150.0, "working", 1_000.0),
        );
        assert_eq!((memo.rows, memo.since_ms), (2, 1_000.0));
    }

    /// The line must leave a small margin: one that would just touch the pills expands.
    #[test]
    fn a_line_that_only_just_fits_expands() {
        // 24 + 120 + 8 + 144 = 296 of 300: inside the 8px margin.
        let memo = next(
            &RowsMemo::default(),
            &input(300.0, 120.0, 144.0, "working", 0.0),
        );
        assert_eq!(memo.rows, 2);
    }

    #[test]
    fn nothing_to_split_stays_one_row() {
        assert_eq!(
            next(
                &RowsMemo::default(),
                &input(200.0, 900.0, 0.0, "working", 0.0)
            )
            .rows,
            1
        );
        assert_eq!(
            next(&RowsMemo::default(), &input(200.0, 0.0, 900.0, "idle", 0.0)).rows,
            1
        );
        let two = RowsMemo {
            rows: 2,
            since_ms: 0.0,
            recheck_at_ms: None,
        };
        assert_eq!(
            next(&two, &input(200.0, 120.0, 0.0, "working", 10.0)).rows,
            1,
            "no pills left"
        );
    }

    /// The tool label changes with every tool; a short one after a long one keeps two rows until
    /// the turn ends.
    #[test]
    fn a_running_turn_never_collapses() {
        let mut memo = next(
            &RowsMemo::default(),
            &input(300.0, 220.0, 150.0, "working", 0.0),
        );
        assert_eq!(memo.rows, 2);
        for (step, label) in [40.0, 220.0, 10.0, 300.0, 60.0].into_iter().enumerate() {
            memo = next(
                &memo,
                &input(300.0, label, 150.0, "working", 10_000.0 * step as f64),
            );
            assert_eq!(memo.rows, 2, "label {label}");
        }
    }

    #[test]
    fn the_turn_ending_collapses_once_everything_fits_with_room() {
        let two = next(
            &RowsMemo::default(),
            &input(300.0, 220.0, 150.0, "working", 0.0),
        );
        // "Worked for 2m" is short: it fits with the collapse margin to spare.
        let after = next(&two, &input(300.0, 60.0, 150.0, "worked", 5_000.0));
        assert_eq!(after.rows, 1);
        // A worked line that still doesn't fit with room keeps two rows.
        let after = next(&two, &input(300.0, 110.0, 150.0, "worked", 5_000.0));
        assert_eq!(after.rows, 2);
    }

    /// A turn ending right after it expanded waits out the hold, and says when to ask again.
    #[test]
    fn two_rows_hold_for_a_moment_before_collapsing() {
        let two = next(
            &RowsMemo::default(),
            &input(300.0, 220.0, 150.0, "working", 1_000.0),
        );
        let held = next(&two, &input(300.0, 60.0, 150.0, "worked", 1_500.0));
        assert_eq!((held.rows, held.recheck_at_ms), (2, Some(3_000.0)));
        let later = next(&held, &input(300.0, 60.0, 150.0, "worked", 3_000.0));
        assert_eq!((later.rows, later.recheck_at_ms), (1, None));
    }

    /// Out of a turn, a pill as wide as the scroll-to-bottom or ? pill coming and going must not flip
    /// the rows back and forth: the collapse margin is wider than such a pill.
    #[test]
    fn a_small_pill_coming_and_going_never_flaps() {
        let arrow = 26.0;
        let mut memo = RowsMemo::default();
        let mut flips = 0;
        let mut last = 1;
        for step in 0..20 {
            let pills = if step % 2 == 0 { 150.0 + arrow } else { 150.0 };
            memo = next(
                &memo,
                &input(300.0, 100.0, pills, "idle", 10_000.0 * step as f64),
            );
            if memo.rows != last {
                flips += 1;
                last = memo.rows;
            }
        }
        assert!(flips <= 1, "flipped {flips} times");
    }

    #[test]
    fn the_query_takes_a_null_memo_and_camel_case_input() {
        let answer = query(&[
            serde_json::Value::Null,
            serde_json::json!({"width": 300, "lead": 24, "label": 220, "pills": 150, "gap": 8, "phase": "working", "nowMs": 5}),
        ])
        .unwrap();
        assert_eq!(answer.rows, 2);
        let json = serde_json::to_value(&answer).unwrap();
        assert_eq!(json["sinceMs"], 5.0);
        let again = query(&[
            json,
            serde_json::json!({"width": 300, "label": 10, "pills": 10, "phase": "working"}),
        ])
        .unwrap();
        assert_eq!(again.rows, 2);
    }
}
