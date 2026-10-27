//! The ? pill's list (chat_pills.rs draws the pill): every question form, page and chart in the
//! chat, under the All / Html / Q&A / Charts filters, in folding sections (Unanswered, For later,
//! Answered and Withdrawn for questions, then Pages and Charts), each row jumping to its card. The sections, their rows and their order are the core's (`openQuestions.sections`,
//! packages/gx-chat-core/src/questions/open_questions.rs, where the user's decision lives); this
//! opens the list on hover, folds its sections and draws it in the pills' menu window
//! (`option_menu/pill_menu.rs`), in the Links card's section look (native_sidebar/links_list.rs).
//!
//! CDXC:SessionChat 2026-10-27 SEE-ALSO:
//! The phone draws the same list in apps/mobile/app/src/chat/native/cards/ChatPills.tsx (`QuestionsMenu`). Change them together.

use super::{appearance::ChatAppearance, state::NativeChatView, transcript::text};
use crate::app::native_chat::cursor::ChatCursor as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, FontFeatures, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Task, Window, div, px, svg,
};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::time::Duration;

/// The list's id among the pill menus, and its trigger's.
pub(super) const QUESTIONS_MENU: &str = "questions";
pub(super) const QUESTIONS_TRIGGER: &str = "chat-pill-questions";
pub(super) const QUESTIONS_MENU_WIDTH: f32 = 440.0;

/// One section header, one row and the list's cap, unscaled: fixed, so the menu knows its height
/// before it opens.
const SECTION_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 26.0;
const LIST_MAX: f32 = 380.0;
/// The space the filters take below the header besides the control itself, which is the modal
/// kit's and does not zoom with the chat.
const FILTERS_GAP: f32 = 2.0;

/// How long the pointer rests on the pill before the list opens, and how long it may be off both
/// the pill and the list before a list it opened closes.
const OPEN_DELAY: Duration = Duration::from_millis(300);
const CLOSE_DELAY: Duration = Duration::from_millis(350);

/// The sections that start folded: the withdrawn ones need nothing.
fn folded_by_default(section: &str) -> bool {
    section == "withdrawn"
}

/// The list's hover and folding, per chat.
#[derive(Default)]
pub(super) struct QuestionList {
    on_pill: bool,
    on_list: bool,
    /// The list on screen was opened by the hover, so leaving closes it again; one opened by a
    /// click stays until the next click or Escape.
    by_hover: bool,
    /// The sections the reader folded or opened against their default.
    toggled: HashSet<String>,
    timer: Option<Task<()>>,
}

impl QuestionList {
    fn folded(&self, section: &str) -> bool {
        folded_by_default(section) ^ self.toggled.contains(section)
    }
}

/// The list's content height (unscaled, at the chat's `scale`) for `open` (the document's
/// `openQuestions`), or `None` when the chat has nothing to list.
pub(super) fn questions_menu_height(open: &Value, list: &QuestionList, scale: f32) -> Option<f32> {
    if open["total"].as_u64().unwrap_or(0) == 0 {
        return None;
    }
    let sections = open["sections"].as_array().cloned().unwrap_or_default();
    let filters = crate::app::window::native_modal_kit::palette::MODAL_CONTROL_HEIGHT
        / scale.max(0.1)
        + FILTERS_GAP
        + super::chat_pills::MENU_SECTION_GAP;
    if sections.is_empty() {
        // The picked filter has nothing: one line says so.
        return Some(
            super::chat_pills::MENU_HEADER_HEIGHT
                + super::chat_pills::MENU_SECTION_GAP
                + filters
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
            + filters
            + rows.min(LIST_MAX),
    )
}

/// What a row is, for its icon and its label: an HTML page, a Q&A form or a chart.
fn kind_look(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "page" => ("titlebar/file-type-html.svg", "HTML"),
        "chart" => ("titlebar/chart-bar.svg", "Chart"),
        _ => ("titlebar/list-check.svg", "Q&A"),
    }
}

impl NativeChatView {
    /// The pointer came onto the ? pill or left it: the list opens after a moment's rest and a
    /// list the hover opened closes once the pointer has left both the pill and the list.
    pub(super) fn question_list_pill_hovered(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.question_list.on_pill = hovered;
        if hovered && !self.chat_menu_is_open(QUESTIONS_TRIGGER) {
            self.question_list.timer = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(OPEN_DELAY).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.question_list.on_pill && !this.chat_menu_is_open(QUESTIONS_TRIGGER) {
                        this.open_questions_menu(true, window, cx);
                    }
                });
            }));
        } else if !hovered {
            self.schedule_question_list_close(window, cx);
        }
    }

    /// The pointer came onto the open list or left it.
    fn question_list_hovered(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.question_list.on_list = hovered;
        if !hovered {
            self.schedule_question_list_close(window, cx);
        }
    }

    fn schedule_question_list_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.question_list.by_hover {
            self.question_list.timer = None;
            return;
        }
        self.question_list.timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CLOSE_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                let list = &this.question_list;
                if list.by_hover
                    && !list.on_pill
                    && !list.on_list
                    && this.chat_menu_is_open(QUESTIONS_TRIGGER)
                {
                    this.close_pill_menu(cx);
                }
            });
        }));
    }

    /// Opens the list over the ? pill (`by_hover`: the hover opened it), or, for a click, shuts the
    /// list the pill already has open.
    pub(super) fn open_questions_menu(
        &mut self,
        by_hover: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if by_hover && self.chat_menu_is_open(QUESTIONS_TRIGGER) {
            return;
        }
        let anchor = self
            .working_card
            .menu_anchor(self.pill_bounds.questions.get());
        let was_open = self.chat_menu_is_open(QUESTIONS_TRIGGER);
        self.toggle_pill_menu(QUESTIONS_MENU, anchor, window, cx);
        self.question_list.by_hover = by_hover && !was_open;
        self.question_list.on_list = false;
    }

    /// The list went away.
    pub(super) fn questions_menu_closed(&mut self) {
        self.question_list.by_hover = false;
        self.question_list.on_list = false;
        self.question_list.timer = None;
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
        let scale = ChatAppearance::current(&self.snapshot).scale;
        questions_menu_height(&self.snapshot["openQuestions"], &self.question_list, scale)
    }

    /// All, Html, Q&A and Charts over the list, each with its count (the core picks the rows).
    fn question_list_filters(&self, open: &Value, cx: &mut Context<Self>) -> AnyElement {
        use crate::app::window::native_modal_kit::segmented_and_rail::{
            ModalRailItem, modal_bordered_segmented_control,
        };
        let filters = open["filters"].as_array().cloned().unwrap_or_default();
        let picked = text(open, "filter");
        let items: Vec<ModalRailItem> = filters
            .iter()
            .map(|filter| ModalRailItem {
                label: text(filter, "label").into(),
                trailing: Some(filter["count"].as_u64().unwrap_or(0).to_string().into()),
                dot: None,
            })
            .collect();
        let selected = filters
            .iter()
            .position(|filter| text(filter, "id") == picked)
            .unwrap_or(0);
        let ids: Vec<String> = filters.iter().map(|filter| text(filter, "id")).collect();
        let ink = gpui::Rgba::from(ChatAppearance::current(&self.snapshot).foreground);
        div()
            .w_full()
            .flex_shrink_0()
            .pb(px(FILTERS_GAP))
            .child(
                modal_bordered_segmented_control(
                    ink,
                    gpui::Rgba { a: 0.14, ..ink },
                    gpui::Rgba { a: 0.55, ..ink },
                    "chat-questions-filters",
                    &items,
                    selected,
                    move |this: &mut Self, index, _window, cx| {
                        if let Some(id) = ids.get(index) {
                            this.invoke(json!({"type": "questionListFilter", "filter": id}), cx);
                        }
                    },
                    cx,
                )
                .w_full(),
            )
            .into_any_element()
    }

    /// The list, drawn in the menu's window with this view's listeners.
    pub(super) fn questions_menu_body(
        &self,
        p: &ChatAppearance,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.snapshot["openQuestions"].clone();
        if open["total"].as_u64().unwrap_or(0) == 0 {
            return None;
        }
        let sections = open["sections"].as_array().cloned().unwrap_or_default();
        let s = p.scale;
        let header = super::chat_pills::pill_menu_header(
            "titlebar/question-mark.svg",
            "Content",
            text(&open, "summary"),
            None,
            p,
        );
        let filters = self.question_list_filters(&open, cx);
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
                .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                    this.question_list_hovered(*hovered, window, cx)
                }))
                .child(super::chat_pills::pill_menu_column(
                    "chat-questions",
                    "Generated content",
                    header,
                    Some(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(super::chat_pills::MENU_SECTION_GAP * s))
                            .child(filters)
                            .child(list)
                            .into_any_element(),
                    ),
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

    /// One form or page: what it is, its title, its tags and when it was asked. A click scrolls
    /// the transcript to its card and flashes it.
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
        let time = text(row, "time");
        let time_title = text(row, "timeTitle");
        let tags = row["tags"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        let (icon, kind_label) = kind_look(&text(row, "kind"));
        let (icon_color, title_color) = match section {
            "answered" => (p.success(), p.foreground),
            "withdrawn" => (p.muted, p.card_muted),
            _ => (p.card_muted, p.foreground),
        };
        let tooltip = [title.as_str(), tags.as_str(), time_title.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let jump = id.clone();
        div()
            .id(SharedString::from(format!("chat-questions-row:{id}")))
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
            // What the row is, beside its title, so pages, forms and charts read apart in any
            // section (an answered row's state is its green icon and its section).
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(5.0 * s))
                    .rounded(px(4.0 * s))
                    .bg(p.foreground.opacity(0.07))
                    .text_size(px(10.5 * s))
                    .text_color(p.card_muted)
                    .child(kind_label),
            )
            .when(!tags.is_empty(), |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .max_w(px(120.0 * s))
                        .truncate()
                        .text_size(px(11.0 * s))
                        .text_color(p.card_muted)
                        .child(tags),
                )
            })
            .when(!time.is_empty(), |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(11.0 * s))
                        .text_color(p.muted)
                        .font_features(FontFeatures(vec![("tnum".into(), 1)].into()))
                        .child(time),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.invoke(json!({"type": "jumpToQuestion", "id": jump}), cx);
                this.close_pill_menu(cx);
            }))
            .into_any_element()
    }
}
