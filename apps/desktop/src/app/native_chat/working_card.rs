//! The working card (`workingCard`, decided in packages/gx-chat-core/src/extras/working_card.rs):
//! the spark, the label and the pills, in its own row at the top of the composer's stack
//! (composer.rs). One line while they fit, else two (the label on top, the pills under it), as
//! gx-chat-core's `extras/working_card_rows.rs` decides from what this measures. It never opens;
//! each pill opens its own menu (chat_pills.rs).
//!
//! CDXC:SessionChat 2026-10-18 SEE-ALSO:
//! The phone draws the same card in apps/mobile/app/src/chat/native/cards/WorkingCard.tsx.

use super::{
    appearance::ChatAppearance,
    cursor::ChatCursor as _,
    state::NativeChatView,
    transcript::text,
    working_spark::{css_ease_in_out, spark},
};
use crate::app::helpers::ThrottledAnimationExt;
use crate::assets::chat_working::VISUAL;
use ghostex_gx_chat_core::extras::working_card_rows::{self, RowsInput, RowsMemo};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Animation, AnimationExt as _, AnyElement, Bounds, Context, FontFeatures,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, canvas, div, point, px, relative, svg,
};
use serde_json::Value;
use std::{cell::Cell, rc::Rc, time::Duration};
use web_time::Instant;

/// The header line's padding above and below (unscaled).
const HEADER_PAD_Y: f32 = 6.0;

/// The gap (unscaled) between the card's bottom edge and the chat box's top edge, when nothing else
/// stacks between them (composer.rs).
///
/// CDXC:SessionChat 2026-10-23 DECISION:
/// User, pointing at the gap between the working card and the chat box: "reduce the margin here to 9px" (2026-10-18; it was 16px: the stack's 8px gap plus the box's own 8px lead-in). User (2026-10-23): "please make the gap at B match the gap at A (for all similar cases also for cards that appear above the chat view composer": every gap in the stack above the composer is the stack's 8px gap (working card to a card, card to card, last card to the box); the box adds no lead-in under a card. Supersedes "a card or notice stacked between them keeps the usual spacing". The phone matches in apps/mobile/app/src/chat/native/cards/WorkingCard.tsx.
pub(super) const WORKING_CARD_GAP: f32 = 9.0;

/// A pill's height (unscaled): its 16px line, 2px above and below and a 1px edge (chat_pills.rs).
/// The line keeps that height with no pill on it, so a pill arriving or leaving never changes the
/// card's height and with it the transcript's visible area above.
const PILL_HEIGHT: f32 = 22.0;

/// The label's own row above the divider when the card takes two rows (unscaled), and the room
/// above and below it.
const TOP_ROW_HEIGHT: f32 = PILL_HEIGHT;
const TOP_ROW_PAD: f32 = 7.0;

/// The room above and below the pills under the divider (unscaled): the same, so they sit evenly.
const PILLS_ROW_PAD: f32 = 7.0;

/// How far the label rises into its row while the card grows (unscaled).
const LABEL_RISE: f32 = 6.0;

/// The width of the fade at a scrolling pill row's edge (unscaled).
const PILLS_FADE: f32 = 24.0;

/// One change between one and two rows, animating for `working_card_rows::ANIMATION_MS`.
#[derive(Clone, Copy)]
struct RowsTransition {
    /// Set by the first frame that draws the change, so a late frame still plays it whole.
    at: Option<Instant>,
    to_two: bool,
    /// Names the animation, so each change plays from its start.
    generation: u64,
}

impl RowsTransition {
    fn running(&self) -> bool {
        self.at
            .is_none_or(|at| at.elapsed() < Duration::from_millis(working_card_rows::ANIMATION_MS))
    }
}

/// What the card measured at its last paint.
#[derive(Default)]
pub(super) struct WorkingCardCells {
    /// The label did not fit its line, so hovering it shows the whole text.
    label_truncated: Rc<Cell<bool>>,
    /// The card's border box, which the pills' menus open above.
    bounds: Rc<Cell<Bounds<Pixels>>>,
    /// The label's natural width on one line, uncut, and what trails it (a compaction's numbers).
    label_width: Rc<Cell<f32>>,
    trailing_width: Rc<Cell<f32>>,
    /// Every pill and timer side by side at their natural widths.
    pills_width: Rc<Cell<f32>>,
    /// One row or two, and since when (gx-chat-core `working_card_rows`).
    rows: Rc<Cell<RowsMemo>>,
    transition: Rc<Cell<Option<RowsTransition>>>,
    /// The scrolling pill row, and whether pills hide past its left and right edges.
    pills_scroll: ScrollHandle,
    fades: Rc<Cell<(bool, bool)>>,
    /// Wakes the card when the rule asked to look again (the two-row hold ending) or an animation
    /// ends, so it settles without anything else redrawing it.
    wake: Option<(u64, gpui::Task<()>)>,
    wake_at: Rc<Cell<Option<f64>>>,
}

impl WorkingCardCells {
    /// The card's border box at its last paint.
    pub(super) fn card_bounds(&self) -> Bounds<Pixels> {
        self.bounds.get()
    }

    /// Where a pill's menu opens against: the pill's left and right edges with the card's top, so
    /// the menu's bottom sits the menu's 4px gap above the card, not above the pill.
    ///
    /// CDXC:SessionChat 2026-10-18 DECISION:
    /// User: "please make these floating menus appear with 4px gap from the working card itself, not from the pill". Anchored to the pill, a pill's menu (Threads, Background and the others) overlapped the card's top edge; it now opens with its bottom edge 4px above the card, keeping its horizontal placement against the pill (chat_pills.rs).
    pub(super) fn menu_anchor(&self, pill: Bounds<Pixels>) -> Bounds<Pixels> {
        let card = self.bounds.get();
        if card.size.height <= px(0.0) {
            return pill;
        }
        Bounds::new(point(pill.left(), card.top()), pill.size)
    }
}

impl NativeChatView {
    /// The card shows: the transcript has rows and the core has something for it, or a timer is armed.
    fn working_card_shown(&self) -> bool {
        let armed = self
            .armed_actions
            .as_array()
            .is_some_and(|actions| !actions.is_empty());
        self.list.item_count() > 0 && (self.snapshot["workingCard"]["visible"] == true || armed)
    }

    /// The card for the top of the composer's stack. While the scroll-to-bottom pill shows, a click
    /// anywhere on the card jumps to the bottom like the pill does; the pills and timers on it keep
    /// their own clicks (each stops its click here).
    ///
    /// CDXC:SessionChat 2026-10-20 DECISION:
    /// User: "i want clicking anywhere on the bar that's above the composer to scroll to the bottom (when the scroll to bottom button is shown) so i dont need to click specifically on that button". With the pill hidden (already at the bottom) the card does nothing on click. The phone taps the same card (apps/mobile/app/src/chat/native/cards/WorkingCard.tsx).
    ///
    /// CDXC:SessionChat 2026-10-18 WHY:
    /// The card is a row of the composer's stack, not a transcript row: a row lives in the cached transcript view (transcript_host.rs), and the card's spark animates for as long as the agent works, so every frame of it would lay out every visible row again, the cost that view exists to avoid. Outside the list, the transcript's visible area ends above it and nothing ever scrolls under it. It stays in that row in a short conversation too; lifting it under the last message left the gap above the composer the user asked about (the removed working_lane.rs, which must not come back).
    pub(super) fn render_working_card(
        &mut self,
        p: &ChatAppearance,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.working_card_shown() {
            return None;
        }
        let card = self.snapshot["workingCard"].clone();
        let left_bottom = super::scroll_bottom::transcript_left_bottom(&self.list);
        let header = self.working_card_header(&card, left_bottom, p, cx);
        self.book_working_card_wake(cx);
        let measured = self.working_card.bounds.clone();
        let decide = self.working_card_rows_decider(&card, p, cx);
        let card = div()
            .id("chat-working-card")
            .relative()
            .role(gpui::Role::Status)
            .aria_label(text(&card, "ariaLabel"))
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .border_1()
            .border_color(p.input_border)
            // A capsule, like the pills it carries (the 2026-10-18 DECISION in the core's
            // extras/working_card.rs); with two rows its corners keep the one-row capsule's radius.
            .rounded(px((PILL_HEIGHT / 2.0 + HEADER_PAD_Y + 1.0) * p.scale))
            // The composer's own fill, so the card and the chat box under it always match.
            .bg(p.composer_background)
            .overflow_hidden()
            .text_color(p.card_muted)
            .when(left_bottom, |card| {
                card.chat_cursor_pointer()
                    .on_click(cx.listener(|chat, _, _, cx| chat.jump_to_bottom(cx)))
            })
            .child(header)
            .child(
                // An absolute child with no insets sits inside the 1px border; one border width
                // out gives the border box.
                canvas(
                    move |bounds, window, cx| {
                        measured.set(bounds);
                        decide(bounds, window, cx);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset(px(-1.0)),
            )
            .into_any_element();
        Some(card)
    }

    /// Scroll-to-bottom on its own over the transcript's bottom edge, while the reader is away from
    /// the bottom and no card shows to carry it.
    pub(super) fn render_lone_scroll_bottom(
        &self,
        p: &ChatAppearance,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.list.item_count() == 0 || self.working_card_shown() {
            return None;
        }
        let pill = self.scroll_bottom_icon_pill(p, cx)?;
        let s = p.scale;
        // The column the transcript's rows use (transcript.rs), the pill at its right end.
        let column = div()
            .w_full()
            .max_w(px(768.0 * s))
            .px(px(16.0 * s))
            .when_some(p.transcript_width, |this, width| {
                this.max_w(relative(1.0)).w(relative(width))
            })
            .when_some(self.held_text_width, |this, width| {
                this.flex_shrink_0().w(px(width)).max_w(px(width))
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .px(px(VISUAL.padding_x * s))
                    .block_mouse_except_scroll()
                    .child(pill),
            );
        Some(
            div()
                .absolute()
                .bottom(px(super::transcript_layout::LAYOUT.end_padding * s))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(column)
                .into_any_element(),
        )
    }

    /// The card's line: the lead glyph and the label on the left, cut with an ellipsis where it
    /// meets the pills, which keep their full width on the right.
    fn working_card_header(
        &self,
        card: &Value,
        left_bottom: bool,
        p: &ChatAppearance,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = p.scale;
        let reduced_motion = crate::app::helpers::gpui_macos_reduce_motion_enabled();
        let kind = card["labelKind"].as_str().unwrap_or_default();
        let activity = &card["activity"];
        let lead = match kind {
            "" => None,
            "activity" => Some(activity_lead(activity, p, reduced_motion)),
            "worked" => Some(
                div()
                    .size(px(VISUAL.spark_box * s))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path("titlebar/check.svg")
                            .size(px(VISUAL.spark_size * s))
                            .text_color(p.muted),
                    )
                    .into_any_element(),
            ),
            _ if card["working"] == true => Some(spark(p, reduced_motion)),
            _ => None,
        };
        let label = card["label"].as_str().map(str::to_owned);
        let chat = cx.entity_id();
        // With two rows the top row shows the label in runs, its file paths shortened (the
        // core's `labelParts`); one row keeps the label as it is.
        let two_row_layout = label.is_some()
            && (self.working_card.rows.get().two_rows()
                || (!reduced_motion
                    && self
                        .working_card
                        .transition
                        .get()
                        .is_some_and(|t| !t.to_two && t.running())));
        let parts = card["labelParts"]
            .as_array()
            .filter(|parts| two_row_layout && !parts.is_empty())
            .cloned();
        let label = label.map(|label| self.working_card_label(label, kind, chat, parts, p));
        let mut trailing = Vec::new();
        if kind == "activity" {
            if let Some(hint) = activity["hint"].as_str() {
                let hint = hint.to_owned();
                trailing.push(
                    div()
                        .id("compaction-messaging-hint")
                        .role(gpui::Role::Button)
                        .aria_label("Messaging during compaction")
                        .size(px(20.0 * s))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .tooltip(move |window, cx| {
                            gpui_component::tooltip::Tooltip::new(hint.clone()).build(window, cx)
                        })
                        .child(
                            svg()
                                .path("titlebar/info-circle.svg")
                                .size(px(14.0 * s))
                                .text_color(p.muted),
                        )
                        .into_any_element(),
                );
            }
            trailing.extend(activity_trailing(activity, p, reduced_motion));
        }
        // Right to left from the card's edge: the panel pills, the question pill, the armed timers,
        // and scroll-to-bottom at the far left, so one appearing moves none of the others. The
        // Links pill sits between Workers and Tasks (links_pill.rs).
        let ids: Vec<&str> = card["pills"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let mut panel_pills: Vec<AnyElement> = ids
            .iter()
            .filter_map(|id| {
                self.snapshot["chatPills"]
                    .as_array()?
                    .iter()
                    .find(|pill| pill["id"] == *id)
            })
            .map(|pill| self.chat_pill(pill, p, cx))
            .collect();
        if let Some(links) = &self.links_pill {
            let at = super::links_pill::links_pill_index(&ids).min(panel_pills.len());
            panel_pills.insert(at, self.links_pill_element(links, p, cx));
        }
        let pills_width = self.working_card.pills_width.clone();
        let pills = div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap(px(6.0 * s))
            .children(
                left_bottom
                    .then(|| self.scroll_bottom_icon_pill(p, cx))
                    .flatten(),
            )
            .children(self.armed_action_items(p, cx))
            .children(self.previous_question_icon_pill(p, cx))
            .children(panel_pills)
            .child(
                canvas(
                    move |bounds, _, _| pills_width.set(f32::from(bounds.size.width)),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        let has_label = label.is_some();
        if !has_label {
            self.working_card.label_width.set(0.0);
        }
        let trailing_width = self.working_card.trailing_width.clone();
        let trailing = (!trailing.is_empty()).then(|| {
            div()
                .relative()
                .flex()
                .items_center()
                .flex_shrink_0()
                .gap(px(VISUAL.gap * s))
                .children(trailing)
                .child(
                    canvas(
                        move |bounds, _, _| trailing_width.set(f32::from(bounds.size.width)),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
        });
        if trailing.is_none() {
            self.working_card.trailing_width.set(0.0);
        }
        // Takes what the pills leave on one line, or its own row on two; empty when the card
        // carries only pills.
        let label_line = div()
            .flex()
            .items_center()
            .gap(px(VISUAL.gap * s))
            .flex_1()
            .min_w_0()
            .children(label)
            .children(trailing);
        let rows = self.working_card.rows.get();
        let mut transition = self.working_card.transition.get();
        if let Some(t) = transition.as_mut().filter(|t| t.at.is_none()) {
            t.at = Some(Instant::now());
            self.working_card.transition.set(Some(*t));
            // Wake when the animation ends, so a collapse settles into one line.
            self.working_card.wake_at.set(Some(
                crate::app::gx_chat::now_millis() as f64
                    + working_card_rows::ANIMATION_MS as f64
                    + 16.0,
            ));
        }
        let reduced = reduced_motion;
        // Two rows show while the rule says so, and through the collapse back to one.
        let collapsing = !reduced && transition.is_some_and(|t| !t.to_two && t.running());
        let two_rows = has_label && (rows.two_rows() || collapsing);
        let padded = |row: gpui::Div| {
            row.w_full()
                .min_w_0()
                .pl(px(12.0 * s))
                .pr(px(10.0 * s))
                .flex()
                .items_center()
                .gap(px(VISUAL.gap * s))
        };
        let animation = |name: &str, generation: u64| {
            (
                gpui::ElementId::Name(format!("chat-working-card-{name}-{generation}").into()),
                // Ease-out cubic, as the phone's (an ease-out quint grew the row before its label
                // showed).
                Animation::new(Duration::from_millis(working_card_rows::ANIMATION_MS))
                    .with_easing(|t: f32| 1.0 - (1.0 - t).powi(3)),
            )
        };
        if !two_rows {
            // One line: the label (fading back in at the end of a collapse) and the pills.
            let label_line = match transition.filter(|t| !reduced && !t.to_two) {
                Some(t) if has_label => {
                    let (id, motion) = animation("label-in", t.generation);
                    label_line
                        .with_animation(id, motion, |line, delta| line.opacity(delta))
                        .into_any_element()
                }
                _ => label_line.into_any_element(),
            };
            let pills_area = self.working_card_pills_area(pills, !has_label, false, p, cx);
            return padded(div())
                .id("chat-working-card-header")
                .min_h(px((PILL_HEIGHT + 2.0 * HEADER_PAD_Y) * s))
                .pt(px(HEADER_PAD_Y * s))
                .pb(px(HEADER_PAD_Y * s))
                .children(lead)
                .when(has_label, |row| row.child(label_line))
                .child(pills_area)
                .into_any_element();
        }
        // Two rows (option C): the lead, the label and the action's time on top; a hairline across
        // the card; the pills under it, from the left (extras/working_card_label.rs has the
        // user's words). The top row and its divider grow in (or shrink away on a collapse) while
        // the pills keep their place at the bottom.
        let elapsed = card["elapsed"].as_str().map(|elapsed| {
            div()
                .flex_shrink_0()
                .text_size(px(12.0 * s))
                .text_color(p.muted)
                .font_features(FontFeatures(vec![("tnum".into(), 1)].into()))
                .child(elapsed.to_owned())
        });
        let top = padded(div())
            .h(px(TOP_ROW_HEIGHT * s))
            .children(lead)
            .child(label_line)
            .children(elapsed);
        let top = div()
            .w_full()
            .flex()
            .flex_col()
            .pt(px(TOP_ROW_PAD * s))
            .pb(px(TOP_ROW_PAD * s))
            .child(top)
            .relative();
        let lane = div()
            .w_full()
            .flex()
            .flex_col()
            .child(top)
            // The hairline between the rows, the card's own border colour, edge to edge.
            .child(div().h(px(1.0)).mx(px(2.0 * s)).bg(p.input_border));
        let lane_height = (TOP_ROW_HEIGHT + 2.0 * TOP_ROW_PAD) * s + 1.0;
        let lane = match transition.filter(|t| !reduced && t.running()) {
            Some(t) => {
                let to_two = t.to_two;
                let (id, motion) = animation(if to_two { "grow" } else { "shrink" }, t.generation);
                div()
                    .w_full()
                    .overflow_hidden()
                    .child(lane)
                    .with_animation(id, motion, move |wrap, delta| {
                        let shown = if to_two { delta } else { 1.0 - delta };
                        wrap.h(px(lane_height * shown)).opacity(shown)
                    })
                    .into_any_element()
            }
            None => lane.into_any_element(),
        };
        let lane = match transition.filter(|t| !reduced && t.to_two && t.running()) {
            // The label rises into its row as it grows.
            Some(t) => {
                let (id, motion) = animation("rise", t.generation);
                div()
                    .w_full()
                    .relative()
                    .child(lane)
                    .with_animation(id, motion, move |wrap, delta| {
                        wrap.top(px(LABEL_RISE * s * (1.0 - delta)))
                    })
                    .into_any_element()
            }
            None => lane,
        };
        div()
            .id("chat-working-card-header")
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .child(lane)
            .child(
                padded(div())
                    .h(px((PILL_HEIGHT + 2.0 * PILLS_ROW_PAD) * s))
                    .py(px(PILLS_ROW_PAD * s))
                    .child(self.working_card_pills_area(pills, true, true, p, cx)),
            )
            .into_any_element()
    }

    /// The pills' area. With room of its own (`fill`: no label beside it, or the bottom of two
    /// rows) it takes the line's width, keeps the pills at its right end while they fit, and
    /// scrolls sideways when they don't, fading them out at an edge that hides more of them; the
    /// card's rounded edge stays whole. Beside a label on one line it is the pills' own width.
    ///
    /// CDXC:SessionChat 2026-10-28 DECISION:
    /// User, of the bar above the composer: "I don't like how the background gets cut off on the right. I think we can do something better than that to show that it's going off screen. I don't like inner shadow." The pills scroll inside the card, inset from its rounded edge, and fade into the card's own fill over 24px at an edge that hides more of them (the right edge until scrolled to the end, the left once scrolled); no fade while everything shows. The phone matches in apps/mobile/app/src/chat/native/cards/WorkingCard.tsx.
    fn working_card_pills_area(
        &self,
        pills: gpui::Div,
        fill: bool,
        from_left: bool,
        p: &ChatAppearance,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = p.scale;
        if !fill {
            return pills.relative().into_any_element();
        }
        let scroll = self.working_card.pills_scroll.clone();
        let fades = self.working_card.fades.clone();
        let (left, right) = fades.get();
        let chat = cx.entity_id();
        let edge = p.composer_background;
        let fade = |toward_left: bool| {
            let ramp = div().absolute().top_0().bottom_0().w(px(PILLS_FADE * s));
            let ramp = if toward_left {
                ramp.left_0()
            } else {
                ramp.right_0()
            };
            ramp.bg(gpui::linear_gradient(
                90.0,
                gpui::linear_color_stop(if toward_left { edge } else { edge.opacity(0.0) }, 0.0),
                gpui::linear_color_stop(if toward_left { edge.opacity(0.0) } else { edge }, 1.0),
            ))
        };
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                div()
                    .id("chat-working-card-pills")
                    .size_full()
                    .flex()
                    .items_center()
                    .overflow_x_scroll()
                    .track_scroll(&scroll)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .when(!from_left, |row| row.justify_end())
                            .flex_shrink_0()
                            .min_w(relative(1.0))
                            .child(pills.relative()),
                    ),
            )
            .when(left, |area| area.child(fade(true)))
            .when(right, |area| area.child(fade(false)))
            .child(
                canvas(
                    move |_, _, cx| {
                        let offset = -f32::from(scroll.offset().x);
                        let max = f32::from(scroll.max_offset().x);
                        let next = (offset > 0.5, max - offset > 0.5);
                        if fades.replace(next) != next {
                            cx.notify(chat);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }

    /// The card's row rule, run once the card has painted and measured its parts: asks
    /// gx-chat-core for one row or two, starts the animation when that changes, and redraws.
    fn working_card_rows_decider(
        &self,
        card: &Value,
        p: &ChatAppearance,
        cx: &Context<Self>,
    ) -> impl Fn(Bounds<Pixels>, &mut gpui::Window, &mut gpui::App) + 'static {
        let s = p.scale;
        let cells = (
            self.working_card.label_width.clone(),
            self.working_card.trailing_width.clone(),
            self.working_card.pills_width.clone(),
            self.working_card.rows.clone(),
            self.working_card.transition.clone(),
            self.working_card.wake_at.clone(),
        );
        // As `working_card_header` picks it: activity and worked always lead with a glyph, a tool
        // or the word only while the agent works.
        let kind = card["labelKind"].as_str().unwrap_or_default();
        let has_lead =
            matches!(kind, "activity" | "worked") || (!kind.is_empty() && card["working"] == true);
        let phase = card["phase"].as_str().unwrap_or_default().to_string();
        let chat = cx.entity_id();
        move |bounds, window, cx| {
            let (label, trailing, pills, rows, transition, wake_at) = &cells;
            let gap = VISUAL.gap * s;
            let trailing = trailing.get();
            let label = label.get();
            let input = RowsInput {
                // The border and the line's padding (12 left, 10 right) come off the card's width.
                width: f32::from(bounds.size.width) - 2.0 - 22.0 * s,
                lead: if has_lead {
                    VISUAL.spark_box * s + gap
                } else {
                    0.0
                },
                label: if label > 0.0 && trailing > 0.0 {
                    label + gap + trailing
                } else {
                    label
                },
                pills: pills.get(),
                gap,
                phase: phase.clone(),
                now_ms: crate::app::gx_chat::now_millis() as f64,
            };
            let before = rows.get();
            let next = working_card_rows::next(&before, &input);
            rows.set(next);
            if next.rows.max(1) != before.rows.max(1) {
                let generation = transition.get().map_or(0, |t| t.generation + 1);
                transition.set(Some(RowsTransition {
                    at: None,
                    to_two: next.two_rows(),
                    generation,
                }));
                // Draw the change on the very next frame (a notify alone waits for the next
                // redraw something else asks for, the spark's throttled beat on the web).
                window.refresh();
                cx.notify(chat);
            } else if next.recheck_at_ms != before.recheck_at_ms && next.recheck_at_ms.is_some() {
                wake_at.set(next.recheck_at_ms);
                window.refresh();
                cx.notify(chat);
            }
        }
    }

    /// Books the card's wake (`WorkingCardCells::wake_at`): one timer at a time, for the latest
    /// time asked.
    fn book_working_card_wake(&mut self, cx: &mut Context<Self>) {
        let Some(at) = self.working_card.wake_at.take() else {
            return;
        };
        let key = at as u64;
        if self
            .working_card
            .wake
            .as_ref()
            .is_some_and(|(booked, _)| *booked == key)
        {
            return;
        }
        let wait = (at - crate::app::gx_chat::now_millis() as f64).max(0.0);
        let task = cx.spawn(async move |chat, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(wait as u64))
                .await;
            let _ = chat.update(cx, |chat, cx| {
                chat.working_card.wake = None;
                cx.notify();
            });
        });
        self.working_card.wake = Some((key, task));
    }

    /// The label on one line, cut with an ellipsis at its end; when it is cut, hovering it shows the
    /// whole text in the app's tooltip.
    fn working_card_label(
        &self,
        label: String,
        kind: &str,
        chat: gpui::EntityId,
        parts: Option<Vec<Value>>,
        p: &ChatAppearance,
    ) -> AnyElement {
        let s = p.scale;
        let truncated = self.working_card.label_truncated.clone();
        let natural = self.working_card.label_width.clone();
        let cut = truncated.get();
        let measured = label.clone();
        let full = label.clone();
        let (size, color) = match kind {
            "tool" | "activity" => (13.0, p.foreground),
            _ => (VISUAL.font_size, p.muted),
        };
        div()
            .id("chat-working-card-label")
            .relative()
            .min_w_0()
            .flex_shrink(1.0)
            .text_size(px(size * s))
            .line_height(relative(1.5))
            .text_color(color)
            .truncate()
            .when(cut, |this| {
                this.tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(full.clone()).build(window, cx)
                })
            })
            .child(match parts {
                // The two-row top row: a path's folders dimmed and shortened, its file name whole.
                Some(parts) => {
                    let mut text = String::new();
                    let mut dimmed = Vec::new();
                    for part in &parts {
                        let run = part["text"].as_str().unwrap_or_default();
                        if part["dim"] == true {
                            dimmed.push((
                                text.len()..text.len() + run.len(),
                                gpui::HighlightStyle {
                                    color: Some(p.muted),
                                    ..Default::default()
                                },
                            ));
                        }
                        text.push_str(run);
                    }
                    gpui::StyledText::new(text)
                        .with_highlights(dimmed)
                        .into_any_element()
                }
                None => label.into_any_element(),
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let style = window.text_style();
                        let font_size = style.font_size.to_pixels(window.rem_size());
                        let run = style.to_run(measured.len());
                        let width = window
                            .text_system()
                            .shape_line(measured.clone().into(), font_size, &[run], None)
                            .width;
                        natural.set(f32::from(width));
                        let cut = width > bounds.size.width + px(0.5);
                        if truncated.replace(cut) != cut {
                            cx.notify(chat);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }

    /// CDXC:DelayedSend 2026-09-21 DECISION:
    /// User: clicking an armed Delayed Send or Close After Done indicator on the chat working row opens the Delayed Actions modal so they can be managed there. (2026-10-17: they ride on the working card, Q259.)
    fn armed_action_items(&self, p: &ChatAppearance, cx: &Context<Self>) -> Vec<AnyElement> {
        let s = p.scale;
        self.armed_actions
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|action| {
                let color = VISUAL.armed_color(action["id"].as_str()?)?;
                let label = action["label"].as_str()?.to_owned();
                let id = action["id"].as_str()?.to_owned();
                Some(
                    div()
                        .id(format!("chat-armed-{id}"))
                        .role(gpui::Role::Button)
                        .aria_label(format!("{label}. Manage delayed actions"))
                        .cursor_pointer()
                        .hover(|style| style.opacity(0.8))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.host(
                                "delayedActions",
                                serde_json::json!({"type": "host", "action": "delayedActions"}),
                                cx,
                            );
                        }))
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .gap(px(VISUAL.armed_icon_gap * s))
                        .child(
                            div()
                                .size(px(VISUAL.spark_box * s))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    svg()
                                        .path("titlebar/clock.svg")
                                        .size(px(VISUAL.spark_size * s))
                                        .text_color(color),
                                ),
                        )
                        .child(
                            div()
                                .whitespace_nowrap()
                                .text_size(px(VISUAL.font_size * s))
                                .line_height(relative(1.5))
                                .text_color(p.foreground)
                                .child(label),
                        )
                        .into_any_element(),
                )
            })
            .collect()
    }
}

/// A live activity's lead: the loader while shells run, else the pulsing dot.
fn activity_lead(activity: &Value, p: &ChatAppearance, reduced_motion: bool) -> AnyElement {
    let s = p.scale;
    let glyph = if activity["shellsRunning"] == true {
        let glyph = svg()
            .path("titlebar/loader2.svg")
            .size(px(14.0 * s))
            .text_color(p.control_primary);
        if reduced_motion {
            glyph.into_any_element()
        } else {
            glyph
                .with_throttled_animation(
                    "chat-shells-running",
                    Duration::from_secs(1),
                    |glyph, progress| {
                        glyph.with_transformation(gpui::Transformation::rotate(gpui::percentage(
                            progress,
                        )))
                    },
                )
                .into_any_element()
        }
    } else {
        let dot = div().size(px(6.0 * s)).rounded_full().bg(p.control_primary);
        if reduced_motion {
            dot.into_any_element()
        } else {
            dot.with_throttled_animation(
                "chat-activity-dot",
                Duration::from_millis(1600),
                |dot, phase| {
                    let progress = css_ease_in_out(if phase < 0.5 {
                        phase * 2.0
                    } else {
                        (1.0 - phase) * 2.0
                    });
                    dot.opacity(1.0 - 0.65 * progress)
                },
            )
            .into_any_element()
        }
    };
    div()
        .size(px(VISUAL.spark_box * s))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(glyph)
        .into_any_element()
}

/// A live activity's clock, tokens, percent and a small progress track after its label.
fn activity_trailing(
    activity: &Value,
    p: &ChatAppearance,
    reduced_motion: bool,
) -> Vec<AnyElement> {
    let s = p.scale;
    let numbers = |value: String| {
        div()
            .flex_shrink_0()
            .text_size(px(VISUAL.font_size * s))
            .text_color(p.muted)
            .font_features(FontFeatures(vec![("tnum".into(), 1)].into()))
            .child(value)
            .into_any_element()
    };
    let mut parts = Vec::new();
    let percent = activity["percent"].as_f64();
    if percent.is_some() || activity["indeterminate"] == true {
        let primary = p.control_primary;
        let track = div()
            .relative()
            .w(px(48.0 * s))
            .h(px(4.0 * s))
            .flex_shrink_0()
            .rounded_full()
            .overflow_hidden()
            .bg(p.foreground.opacity(0.1));
        let fill = div().h_full().rounded_full().bg(primary);
        parts.push(if let Some(percent) = percent {
            track
                .child(fill.w(relative(percent as f32 / 100.0)))
                .into_any_element()
        } else if reduced_motion {
            track
                .flex()
                .justify_center()
                .child(fill.w(relative(0.35)))
                .into_any_element()
        } else {
            track
                .with_throttled_animation(
                    "chat-compaction-progress",
                    Duration::from_millis(1800),
                    move |track, phase| {
                        track.child(
                            div()
                                .absolute()
                                .h_full()
                                .rounded_full()
                                .bg(primary)
                                .w(relative(0.35))
                                .left(relative(-0.35 + 1.351 * css_ease_in_out(phase))),
                        )
                    },
                )
                .into_any_element()
        });
    }
    if let Some(elapsed) = activity["elapsedLabel"].as_str() {
        parts.push(numbers(elapsed.to_owned()));
    }
    if let Some(tokens) = activity["tokens"].as_str() {
        parts.push(numbers(tokens.to_owned()));
    }
    if let Some(percent) = percent {
        parts.push(numbers(format!("{percent}%")));
    }
    parts
}
