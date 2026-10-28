//! The Resources pill's list (chat_pills.rs draws the pill; a click opens it, a second click or
//! Escape closes it, and the ? pill never opens it): every question form, page and chart in the
//! chat, under the All / Html / Q&A / Charts filter (a small dropdown on the header's line), in
//! folding sections (Unanswered, For later, Answered and Withdrawn for questions, then Pages and
//! Charts), each row jumping to its card. The sections, their rows and their order are the core's
//! (`openQuestions.sections`,
//! packages/gx-chat-core/src/questions/open_questions.rs, where the user's decision lives); this
//! folds its sections and draws it in the pills' menu window
//! (`option_menu/pill_menu.rs`), in the Links card's section look (native_sidebar/links_list.rs).
//!
//! CDXC:SessionChat 2026-10-27 SEE-ALSO:
//! The phone draws the same list in apps/mobile/app/src/chat/native/cards/ChatPills.tsx (`QuestionsMenu`). Change them together.

use super::{appearance::ChatAppearance, state::NativeChatView, transcript::text};
use crate::app::native_chat::cursor::ChatCursor as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, FontFeatures, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px, svg,
};
use serde_json::{Value, json};
use std::collections::HashSet;

/// The list's id among the pill menus, and its trigger's.
pub(super) const QUESTIONS_MENU: &str = "resources";
pub(super) const QUESTIONS_TRIGGER: &str = "chat-pill-resources";
pub(super) const QUESTIONS_MENU_WIDTH: f32 = 440.0;

/// One section header, one row and the list's cap, unscaled: fixed, so the menu knows its height
/// before it opens.
const SECTION_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 26.0;
const LIST_MAX: f32 = 380.0;

/// How often an open list redraws so its ages stay current.
const AGE_REFRESH: std::time::Duration = std::time::Duration::from_secs(30);

/// The sections that start folded: the withdrawn ones need nothing.
fn folded_by_default(section: &str) -> bool {
    section == "withdrawn"
}

/// A row's hover tooltip: its title and the full date and time it was asked. No Q numbers
/// anywhere in the list (user, 2026-10-28: "lets also not show the Q numbers in the question
/// tooltip when hovering on them").
fn row_tooltip(row: &Value) -> String {
    [text(row, "title"), text(row, "timeTitle")]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// How long ago a row was asked, worded as of `now_ms` from its `atMs`; the document's own `age`
/// when the row carries no stamp (it was worded when the document was built).
fn row_age(row: &Value, now_ms: i64) -> String {
    match row["atMs"].as_i64() {
        Some(at) => ghostex_gx_chat_core::transcript::message_time::age_label(now_ms, at),
        None => text(row, "age"),
    }
}

/// The button a row offers: "For later" on a form or page still waiting, "Back to open" on one
/// put off. It sends the action the question cards' own button sends (`questionForLater`), so
/// the cards and the list agree. Other rows (answered, withdrawn, pages and charts that ask
/// nothing) offer none.
fn later_action(section: &str, kind: &str, id: &str) -> Option<(&'static str, Value)> {
    if kind == "chart" {
        return None;
    }
    match section {
        "unanswered" => Some(("For later", json!({"type": "questionForLater", "id": id, "later": true}))),
        "later" => Some((
            "Back to open",
            json!({"type": "questionForLater", "id": id, "later": false}),
        )),
        _ => None,
    }
}

/// The list's folding, per chat.
#[derive(Default)]
pub(super) struct QuestionList {
    /// The sections the reader folded or opened against their default.
    toggled: HashSet<String>,
    /// Redraws the open list every 30 s so the ages on its rows keep up (dropped with the list).
    tick: Option<gpui::Task<()>>,
    /// The filter dropdown's trigger, in screen space, which its pick list opens below.
    filter_bounds: std::rc::Rc<std::cell::Cell<gpui::Bounds<gpui::Pixels>>>,
}

impl QuestionList {
    fn folded(&self, section: &str) -> bool {
        folded_by_default(section) ^ self.toggled.contains(section)
    }
}

/// The list's content height (unscaled) for `open` (the document's
/// `openQuestions`), or `None` when the chat has nothing to list.
pub(super) fn questions_menu_height(open: &Value, list: &QuestionList) -> Option<f32> {
    if open["total"].as_u64().unwrap_or(0) == 0 {
        return None;
    }
    let sections = open["sections"].as_array().cloned().unwrap_or_default();
    if sections.is_empty() {
        // The picked filter has nothing: one line says so.
        return Some(
            super::chat_pills::MENU_HEADER_HEIGHT
                + super::chat_pills::MENU_SECTION_GAP
                + ROW_HEIGHT,
        );
    }
    let rows: f32 = sections
        .iter()
        .map(|section| {
            let shown = if list.folded(&text(section, "id")) {
                0
            } else {
                section["rows"].as_array().map_or(0, Vec::len)
            };
            SECTION_HEIGHT + shown as f32 * ROW_HEIGHT
        })
        .sum();
    Some(
        super::chat_pills::MENU_HEADER_HEIGHT
            + super::chat_pills::MENU_SECTION_GAP
            + rows.min(LIST_MAX),
    )
}

/// The glyph of a row's For later button: a clock to put it off, the way back once it is.
fn later_icon(section: &str) -> &'static str {
    if section == "later" {
        "titlebar/arrow-back-up.svg"
    } else {
        "titlebar/clock.svg"
    }
}

/// What a row is, by its icon: an HTML page, a Q&A form or a chart.
fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "page" => "titlebar/file-type-html.svg",
        "chart" => "titlebar/chart-bar.svg",
        _ => "titlebar/list-check.svg",
    }
}

impl NativeChatView {
    /// While the list is open, redraws it every 30 s: the ages on its rows are worded from the
    /// clock, not from a document change.
    pub(super) fn keep_question_ages_current(&mut self, cx: &mut Context<Self>) {
        self.question_list.tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(AGE_REFRESH).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
    }

    /// The list went away: its refresh stops.
    pub(super) fn questions_menu_closed(&mut self) {
        self.question_list.tick = None;
    }

    fn toggle_question_section(&mut self, section: String, cx: &mut Context<Self>) {
        if !self.question_list.toggled.remove(&section) {
            self.question_list.toggled.insert(section);
        }
        cx.notify();
    }

    /// The list's section a form or page is in (`unanswered`, `later`, `answered`, `withdrawn`).
    pub(super) fn question_section_of(&self, id: &str) -> Option<String> {
        self.snapshot["openQuestions"]["sections"]
            .as_array()?
            .iter()
            .find(|section| {
                section["rows"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|row| row["id"] == id))
            })
            .map(|section| text(section, "id"))
    }

    pub(super) fn questions_menu_height(&self) -> Option<f32> {
        questions_menu_height(&self.snapshot["openQuestions"], &self.question_list)
    }

    /// The filter's small dropdown at the header's right end: the current pick and a chevron. It
    /// opens the chat menus' own pick list below it (the one a panel's select opens), with each
    /// filter's count.
    ///
    /// CDXC:SessionChat 2026-10-27 DECISION:
    /// User, of Chat Resources: "remove pill on the right, the icon on the left is enough indicator. and instead of tabs up top just do a small dropdown (should be top right of the pop up aligned with the header there". The All / Html / Q&A / Charts tabs became this dropdown on the header's line, and rows carry only their kind icon (no HTML / Q&A / Chart label). The phone does the same in ChatPills.tsx.
    fn question_list_filter_trigger(
        &self,
        open: &Value,
        p: &ChatAppearance,
        menu_origin: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = p.scale;
        let filters = open["filters"].as_array().cloned().unwrap_or_default();
        let picked = text(open, "filter");
        let label = filters
            .iter()
            .find(|filter| text(filter, "id") == picked)
            .map(|filter| text(filter, "label"))
            .unwrap_or_else(|| "All".to_string());
        let bounds = self.question_list.filter_bounds.clone();
        let measured = bounds.clone();
        div()
            .id("chat-questions-filter")
            .role(gpui::Role::ComboBox)
            .aria_label(format!("Show: {label}"))
            .flex_shrink_0()
            .relative()
            .flex()
            .items_center()
            .gap(px(4.0 * s))
            .h(px(20.0 * s))
            .px(px(6.0 * s))
            .rounded(px(5.0 * s))
            .text_size(px(12.0 * s))
            .text_color(p.card_muted)
            .chat_cursor_pointer()
            .hover(|style| {
                style
                    .bg(p.foreground.opacity(0.06))
                    .text_color(p.foreground)
            })
            .child(label)
            .child(
                svg()
                    .path("titlebar/chevron-down.svg")
                    .size(px(11.0 * s))
                    .text_color(p.card_muted),
            )
            .child(
                gpui::canvas(
                    move |rect: gpui::Bounds<gpui::Pixels>, _, _| {
                        measured.set(gpui::Bounds::new(menu_origin + rect.origin, rect.size))
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.open_question_list_filters(cx)
            }))
            .into_any_element()
    }

    /// The filter's pick list, below its trigger with their right edges lined up, so it opens
    /// leftwards inside the popup. Picking keeps the list up.
    fn open_question_list_filters(&mut self, cx: &mut Context<Self>) {
        let open = &self.snapshot["openQuestions"];
        let picked = text(open, "filter");
        let rows: Vec<Value> = open["filters"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|filter| {
                json!({
                    "label": filter["label"],
                    "detail": filter["count"].as_u64().unwrap_or(0).to_string(),
                    "checked": text(filter, "id") == picked,
                    "keepOpen": true,
                    "command": {"type": "questionListFilter", "filter": filter["id"]},
                })
            })
            .collect();
        let anchor = self.question_list.filter_bounds.get();
        if let Some(menu) = self.option_menu.clone() {
            menu.update(cx, |menu, cx| {
                menu.open_select_list_right_aligned(rows, anchor, 160.0, 1, cx)
            });
        }
    }

    /// The list, drawn in the menu's window with this view's listeners.
    pub(super) fn questions_menu_body(
        &self,
        p: &ChatAppearance,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.snapshot["openQuestions"].clone();
        if open["total"].as_u64().unwrap_or(0) == 0 {
            return None;
        }
        let sections = open["sections"].as_array().cloned().unwrap_or_default();
        let s = p.scale;
        let trigger = self.question_list_filter_trigger(&open, p, window.bounds().origin, cx);
        let header = super::chat_pills::pill_menu_header(
            "titlebar/question-mark.svg",
            "Chat Resources",
            text(&open, "summary"),
            Some(trigger),
            p,
        );
        let mut list = div()
            .id("chat-questions-list")
            .flex()
            .flex_col()
            .flex_shrink_0()
            .max_h(px(LIST_MAX * s))
            .overflow_y_scroll();
        if sections.is_empty() {
            list = list.child(
                div()
                    .h(px(ROW_HEIGHT * s))
                    .flex()
                    .items_center()
                    .px(px(4.0 * s))
                    .text_size(px(12.0 * s))
                    .text_color(p.card_muted)
                    .child("Nothing of this kind in this chat yet."),
            );
        }
        for section in &sections {
            let id = text(section, "id");
            let folded = self.question_list.folded(&id);
            list = list.child(self.question_section_header(section, folded, p, cx));
            if !folded {
                for row in section["rows"].as_array().into_iter().flatten() {
                    list = list.child(self.question_list_row(&id, row, p, cx));
                }
            }
        }
        Some(
            div()
                .id("chat-questions-menu")
                .w_full()
                .child(super::chat_pills::pill_menu_column(
                    "chat-questions",
                    "Chat Resources",
                    header,
                    Some(list.into_any_element()),
                    None,
                    p,
                ))
                .into_any_element(),
        )
    }

    /// A section's header: the chevron, its title and its count, folding the section on a click.
    fn question_section_header(
        &self,
        section: &Value,
        folded: bool,
        p: &ChatAppearance,
        cx: &Context<Self>,
    ) -> AnyElement {
        let s = p.scale;
        let id = text(section, "id");
        let title = text(section, "title");
        let count = section["count"].as_u64().unwrap_or(0);
        let toggled = id.clone();
        div()
            .id(SharedString::from(format!("chat-questions-section:{id}")))
            .role(gpui::Role::Button)
            .aria_label(format!("{title} {count}"))
            .aria_expanded(!folded)
            .flex_shrink_0()
            .h(px(SECTION_HEIGHT * s))
            .flex()
            .items_center()
            .gap(px(5.0 * s))
            .px(px(4.0 * s))
            .text_size(px(11.0 * s))
            .text_color(p.card_muted)
            .chat_cursor_pointer()
            .hover(|style| style.text_color(p.foreground))
            .child(
                svg()
                    .path(if folded {
                        "titlebar/chevron-right.svg"
                    } else {
                        "titlebar/chevron-down.svg"
                    })
                    .size(px(11.0 * s))
                    .flex_shrink_0()
                    .text_color(p.card_muted),
            )
            .child(title)
            .child(count.to_string())
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_question_section(toggled.clone(), cx)
            }))
            .into_any_element()
    }

    /// One form or page: what it is, its title and when it was asked (its tags are only in the
    /// hover tooltip). A click scrolls the transcript to its card and flashes it.
    /// A row's small icon button (shown while the pointer is on the row) that puts it off For later
    /// or back to open. Its press stops here, so it never also jumps to the question.
    #[allow(clippy::too_many_arguments)]
    fn later_row_button(
        &self,
        id: &str,
        label: &'static str,
        action: Value,
        group: SharedString,
        icon: &'static str,
        p: &ChatAppearance,
        cx: &Context<Self>,
    ) -> AnyElement {
        let s = p.scale;
        div()
            .id(SharedString::from(format!("chat-questions-later:{id}")))
            .role(gpui::Role::Button)
            .aria_label(label)
            .flex_shrink_0()
            .size(px(20.0 * s))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0 * s))
            .opacity(0.0)
            .group_hover(group, |this| this.opacity(1.0))
            .chat_cursor_pointer()
            .hover(|style| style.bg(p.foreground.opacity(0.1)))
            .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(label).build(window, cx))
            .child(
                svg()
                    .path(icon)
                    .size(px(13.0 * s))
                    .text_color(p.card_muted),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.invoke(action.clone(), cx);
            }))
            .into_any_element()
    }

    fn question_list_row(
        &self,
        section: &str,
        row: &Value,
        p: &ChatAppearance,
        cx: &Context<Self>,
    ) -> AnyElement {
        let s = p.scale;
        let id = text(row, "id");
        let title = text(row, "title");
        let age = row_age(row, crate::app::gx_chat::now_millis());
        let kind = text(row, "kind");
        let later = later_action(section, &kind, &id);
        let icon = kind_icon(&kind);
        let group = SharedString::from(format!("chat-questions-row-group:{id}"));
        let (icon_color, title_color) = match section {
            "answered" => (p.success(), p.foreground),
            "withdrawn" => (p.muted, p.card_muted),
            _ => (p.card_muted, p.foreground),
        };
        let tooltip = row_tooltip(row);
        let jump = id.clone();
        div()
            .id(SharedString::from(format!("chat-questions-row:{id}")))
            .group(group.clone())
            .role(gpui::Role::Button)
            .aria_label(format!("Go to {title}"))
            .flex()
            .items_center()
            .gap(px(8.0 * s))
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .h(px(ROW_HEIGHT * s))
            .pl(px(20.0 * s))
            .pr(px(4.0 * s))
            .rounded(px(5.0 * s))
            .text_size(px(12.0 * s))
            .chat_cursor_pointer()
            .hover(|style| style.bg(p.foreground.opacity(0.06)))
            .tooltip(move |window, cx| {
                gpui_component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
            })
            .child(
                svg()
                    .path(icon)
                    .size(px(13.0 * s))
                    .flex_shrink_0()
                    .text_color(icon_color),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(title_color)
                    .when(section == "withdrawn", |title| title.line_through())
                    .child(title),
            )
            .when(!age.is_empty(), |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(11.0 * s))
                        .text_color(p.muted)
                        .font_features(FontFeatures(vec![("tnum".into(), 1)].into()))
                        .child(age),
                )
            })
            .children(later.map(|(label, action)| {
                self.later_row_button(&id, label, action, group.clone(), later_icon(section), p, cx)
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.invoke(json!({"type": "jumpToQuestion", "id": jump}), cx);
                this.close_pill_menu(cx);
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_has_the_title_and_full_time_but_no_q_numbers() {
        let row = json!({
            "title": "Run 4, part 3 of 3",
            "tags": ["Q633", "Q634"],
            "time": "1:57 AM",
            "timeTitle": "1:57 AM, 28th October 2026",
        });
        let tooltip = row_tooltip(&row);
        assert_eq!(tooltip, "Run 4, part 3 of 3\n1:57 AM, 28th October 2026");
        assert!(!tooltip.contains("Q633"));
    }

    #[test]
    fn tooltip_skips_empty_parts() {
        let row = json!({"title": "Page", "tags": [], "time": "", "timeTitle": ""});
        assert_eq!(row_tooltip(&row), "Page");
    }

    #[test]
    fn the_age_is_worded_from_the_stamp_as_the_clock_moves() {
        let row = json!({"atMs": 1_000_000, "age": "stale", "time": "1:57 AM"});
        assert_eq!(row_age(&row, 1_000_000 + 30_000), "now");
        assert_eq!(row_age(&row, 1_000_000 + 4 * 60_000), "4m");
        assert_eq!(row_age(&row, 1_000_000 + 2 * 3_600_000), "2h");
        assert_eq!(row_age(&row, 1_000_000 + 30 * 3_600_000), "yesterday");
        assert_eq!(row_age(&row, 1_000_000 + 3 * 86_400_000), "3d");
        let unstamped = json!({"age": "5m"});
        assert_eq!(row_age(&unstamped, 9_999_999_999), "5m");
        assert_eq!(row_age(&json!({}), 5), "");
    }

    #[test]
    fn the_for_later_button_toggles_by_section() {
        let (label, action) = later_action("unanswered", "form", "f1").unwrap();
        assert_eq!(label, "For later");
        assert_eq!(action, json!({"type": "questionForLater", "id": "f1", "later": true}));
        let (label, action) = later_action("later", "page", "p1").unwrap();
        assert_eq!(label, "Back to open");
        assert_eq!(action, json!({"type": "questionForLater", "id": "p1", "later": false}));
        for section in ["answered", "withdrawn", "pages", "charts"] {
            assert!(later_action(section, "form", "x").is_none(), "{section}");
        }
        assert!(later_action("unanswered", "chart", "c").is_none());
        assert_eq!(later_icon("later"), "titlebar/arrow-back-up.svg");
        assert_eq!(later_icon("unanswered"), "titlebar/clock.svg");
    }
}
