//! The "Previous question" pill and the jumps to a question: which forms and question pages are
//! still waiting on the reader, which one each click of the pill goes to, and the transcript row a
//! Q-tag link lands on. What counts as asked, answered and obsolete is
//! `transcript/open_questions.rs`; the renderers scroll to `jump.item` once per `jump.revision` and
//! highlight `jump.target` briefly.
//!
//! CDXC:SessionChat 2026-10-14 DECISION:
//! User (Q81 = C): a "Previous question ↑" pill next to Scroll to bottom while the chat has unanswered questions; each click scrolls to the next older unanswered one, wrapping, and highlights it briefly. A form whose answers are queued or sending is no longer waiting on the reader. Then: "I don't like the emoji in the Previous question thing. Remove that icon and remove the number." Supersedes the same day's "❓ 2 · Previous question ↑" pill. Q-tag links only appear where the agent names a question and do nothing until clicked.
//!
//! CDXC:SessionChat 2026-10-24 DECISION:
//! User: "please remove the setting that alows disabling/enabling previous questions button". The pill always shows while a question is unanswered; the `chatPreviousQuestion` setting (off by default since 2026-10-14, for testing) is gone, and a value saved in an old settings file is ignored. Supersedes the 2026-10-14 off-by-default switch.

//!
//! CDXC:SessionChat 2026-10-27 DECISION:
//! User: "i want hovering over the ? mark "Previous questions" pill in the chat view to show me a list of all the questions. should look like this threads one in terms of having sections and i really care about the "already answered ones" so i can click to jump to those previously answered ones. make sure we sort by date". The pill carries a list of every question form and question page in the chat (`sections`), in the Links card's folding sections: Unanswered, For later, Answered and Withdrawn, each with its count, rows newest first, each row the page's title (or the form's first question), its tags and its time; a row jumps to its card and flashes it. The pill stays while the chat has any question, so the list is always reachable, but its count, attention dot and ping are only for unanswered ones (`count`). On the desktop hovering opens the list and a click still steps to the previous unanswered question (opening the list when none waits); on the phone a tap opens it.
//!
//! CDXC:SessionChat 2026-10-27 DECISION:
//! User, of the list: "we're not indicating HTML pages from normal questions here", "also please show "All" "Html" "Q&A" "Charts" as 4 selections so i can jump to previous generated content", "also i wanna see even Htmls that don't have any questions in them so i can jump back to them". Every row says what it is (`kind`: an HTML page, a Q&A form or a chart), and four filters sit over the list (`filters`, the reader's pick `filter`): All, Html (pages, with or without questions), Q&A (```questions forms) and Charts (```visual drawings: charts, stats, tables). Forms and question pages keep the Unanswered, For later, Answered and Withdrawn sections; pages that ask nothing are listed under Pages and charts under Charts, newest first. The pill's count, dot and ping stay about unanswered questions only.

use serde::{Deserialize, Serialize};

use crate::effect::Effect;
use crate::state::{ChatContext, ChatState};
use crate::transcript::open_questions::{tag_target, Asker, AskerKind};
use crate::transcript::page_answers::PageAnswers;

/// The document's `openQuestions`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenQuestions {
    /// How many forms and question pages are unanswered; the pill shows while this is above zero.
    /// Zero while the Previous question setting is off, so the pill never shows.
    pub count: u32,
    /// The pill's text: "Previous question ↑".
    pub text: String,
    /// What a screen reader says for it.
    pub accessibility_label: String,
    /// The newest unanswered form's or page's id, or `null` when none waits. The renderers ping the
    /// pill when this changes without `count` dropping: a new question arrived.
    pub newest: Option<String>,
    /// The last jump, or `null` before the first.
    pub jump: Option<OpenQuestionJump>,
    /// Every form and question page in the list; the pill shows while this is above zero.
    #[serde(default)]
    pub total: u32,
    /// The list's header detail: "2 unanswered · 1 for later".
    #[serde(default)]
    pub summary: String,
    /// The list's sections in order for the picked filter, each only when it has rows.
    #[serde(default)]
    pub sections: Vec<QuestionListSection>,
    /// The filters over the list, All, Html, Q&A and Charts, each with its count.
    #[serde(default)]
    pub filters: Vec<QuestionListFilter>,
    /// The picked filter's id (`all`, `html`, `qa`, `charts`); `questionListFilter` picks one.
    #[serde(default)]
    pub filter: String,
}

/// One of the list's filters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionListFilter {
    pub id: String,
    pub label: String,
    pub count: u32,
}

/// One folding section of the pill's list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionListSection {
    /// `unanswered`, `later`, `answered`, `withdrawn`, `pages` or `charts`.
    pub id: String,
    pub title: String,
    pub count: u32,
    /// Newest first.
    pub rows: Vec<QuestionListRow>,
}

/// One form, page or chart in the list; a click sends `jumpToQuestion {id}`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionListRow {
    /// The form's id, the page's id, or the chart's (`questions/content.rs` `chart_id`).
    pub id: String,
    /// `form` (Q&A), `page` (HTML) or `chart`.
    pub kind: String,
    pub title: String,
    /// The Q-tags it asks.
    pub tags: Vec<String>,
    /// When it was asked, as a message's time reads ("14:03", "yesterday at 09:12"); empty when
    /// the transcript does not say.
    pub time: String,
    /// The full date, for a tooltip.
    pub time_title: String,
}

/// One jump to a question: scroll to `item` once per `revision`, then highlight `target` (the
/// form's id or the page's id).
///
/// CDXC:SessionChat 2026-10-20 WHY:
/// Where a jump lands, on every renderer: a reply that fits on screen is shown whole; a taller one has its end at the bottom of the transcript, unless that leaves the question above the top, and then the question's top sits `questionJumpMargin` (`scroll-bottom.json`) below the transcript's top. The question is the form question asking `question` when the form shows it, else the whole form or page card. The renderers measure the card inside the row after the row is drawn, so long unmeasured rows above it do not matter. Supersedes always aligning a tall reply's end, which left a form near the top of a long reply above the screen (user: "because im being scrolled to the bottom of the message and its sometimes loing").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenQuestionJump {
    pub revision: u64,
    pub item: usize,
    pub target: String,
    /// The tag a Q-tag link named, so the jump lands on that question inside the form; `null` for
    /// the pill's jumps, which land on the card.
    pub question: Option<String>,
}

/// What the pill remembers between clicks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenQuestionsState {
    /// The question the last jump went to, which the next click steps back from.
    pub last: Option<String>,
    pub jump: Option<OpenQuestionJump>,
    /// The list's picked filter; empty is All.
    pub filter: String,
}

const FILTERS: [(&str, &str); 4] = [
    ("all", "All"),
    ("html", "Html"),
    ("qa", "Q&A"),
    ("charts", "Charts"),
];

/// Whether the filter `filter` shows a row of kind `kind` (`form`, `page`, `chart`).
fn filter_shows(filter: &str, kind: &str) -> bool {
    match filter {
        "html" => kind == "page",
        "qa" => kind == "form",
        "charts" => kind == "chart",
        _ => true,
    }
}

/// `questionListFilter {filter}`: the list shows what the filter picks.
pub fn pick_filter(state: &mut ChatState, filter: &str) {
    let filter = if FILTERS.iter().any(|(id, _)| *id == filter) && filter != "all" {
        filter.to_string()
    } else {
        String::new()
    };
    if state.questions.open_questions.filter != filter {
        state.questions.open_questions.filter = filter;
        state.core.request_publish();
    }
}

/// The forms and question pages still waiting on the reader, oldest first, less those put off For
/// later (`questions/later.rs`).
pub fn unanswered(state: &ChatState) -> Vec<&Asker> {
    let later = crate::questions::later::later_ids(state);
    waiting(state)
        .into_iter()
        .filter(|asker| !later.contains(&asker.id))
        .collect()
}

/// The forms and question pages no answer, withdrawal or later question has settled yet, For later
/// ones included, oldest first.
pub fn waiting(state: &ChatState) -> Vec<&Asker> {
    state
        .transcript_view
        .askers
        .iter()
        .filter(|asker| !asker.answered && !asker.obsolete && !asker.inherited)
        .filter(|asker| {
            asker.kind != AskerKind::Form
                || state
                    .questions
                    .forms
                    .get(&asker.id)
                    .is_none_or(|form| form.send.is_none() && form.sent.is_none())
        })
        .collect()
}

/// The forms and pages put off For later that still wait: their cards say For later
/// (the document's `laterQuestions`).
pub fn later_waiting(state: &ChatState) -> Vec<String> {
    let later = crate::questions::later::later_ids(state);
    waiting(state)
        .into_iter()
        .filter(|asker| later.contains(&asker.id))
        .map(|asker| asker.id.clone())
        .collect()
}

/// Which section of the list an asker belongs in, or `None` for one this chat does not list (an
/// unanswered question inherited from the session this one was forked from).
fn section_of(
    asker: &Asker,
    waiting: &[&Asker],
    later: &std::collections::BTreeSet<String>,
    on_the_way: &[String],
) -> Option<&'static str> {
    if waiting.iter().any(|known| known.id == asker.id) {
        return Some(if later.contains(&asker.id) {
            "later"
        } else {
            "unanswered"
        });
    }
    if asker.answered || on_the_way.contains(&asker.id) {
        return Some("answered");
    }
    if asker.inherited {
        return None;
    }
    // Neither waiting nor answered: withdrawn, asked again later, or a form whose answers are on
    // their way (answered, as the pill counts it).
    if asker.obsolete {
        Some("withdrawn")
    } else {
        Some("answered")
    }
}

const SECTIONS: [(&str, &str); 6] = [
    ("unanswered", "Unanswered"),
    ("later", "For later"),
    ("answered", "Answered"),
    ("withdrawn", "Withdrawn"),
    ("pages", "Pages"),
    ("charts", "Charts"),
];

/// The pill's list: every form, page and chart in its section, newest first, all kinds.
pub fn list(state: &ChatState, context: &ChatContext) -> Vec<QuestionListSection> {
    let waiting = waiting(state);
    let later = crate::questions::later::later_ids(state);
    let on_the_way: Vec<String> = answers_on_the_way(state)
        .into_iter()
        .map(|answers| answers.page_id)
        .collect();
    let placed: Vec<(&'static str, usize, &Asker)> = state
        .transcript_view
        .askers
        .iter()
        .enumerate()
        .filter_map(|(index, asker)| {
            Some((
                section_of(asker, &waiting, &later, &on_the_way)?,
                index,
                asker,
            ))
        })
        .collect();
    let mut placed: Vec<(&'static str, Option<i64>, usize, QuestionListRow)> = placed
        .into_iter()
        .map(|(section, index, asker)| (section, asker.at, index, list_row(asker, context)))
        .collect();
    let askers = state.transcript_view.askers.len();
    for (index, item) in state.questions.content.items.iter().enumerate() {
        let section = match item.kind {
            crate::transcript::question_forms::ContentKind::Chart => "charts",
            _ => "pages",
        };
        placed.push((section, item.at, askers + index, content_row(item, context)));
    }
    // Newest first; a row the transcript gave no time sorts by its place.
    placed.sort_by(|(_, a_at, a_index, _), (_, b_at, b_index, _)| {
        b_at.cmp(a_at).then(b_index.cmp(a_index))
    });
    SECTIONS
        .iter()
        .filter_map(|(id, title)| {
            let rows: Vec<QuestionListRow> = placed
                .iter()
                .filter(|(section, _, _, _)| section == id)
                .map(|(_, _, _, row)| row.clone())
                .collect();
            (!rows.is_empty()).then(|| QuestionListSection {
                id: id.to_string(),
                title: title.to_string(),
                count: rows.len() as u32,
                rows,
            })
        })
        .collect()
}

fn list_row(asker: &Asker, context: &ChatContext) -> QuestionListRow {
    let time = crate::transcript::message_time::message_time(asker.at, context);
    let title = if asker.title.is_empty() {
        match asker.kind {
            AskerKind::Form => "Questions".to_string(),
            AskerKind::Page => "Page".to_string(),
        }
    } else {
        asker.title.clone()
    };
    QuestionListRow {
        id: asker.id.clone(),
        kind: match asker.kind {
            AskerKind::Form => "form",
            AskerKind::Page => "page",
        }
        .to_string(),
        title,
        tags: asker.tags.clone(),
        time: time["label"].as_str().unwrap_or_default().to_string(),
        time_title: time["title"].as_str().unwrap_or_default().to_string(),
    }
}

fn content_row(
    item: &crate::questions::content::ContentItem,
    context: &ChatContext,
) -> QuestionListRow {
    use crate::transcript::question_forms::ContentKind;
    let time = crate::transcript::message_time::message_time(item.at, context);
    let (kind, fallback) = match item.kind {
        ContentKind::Chart => ("chart", "Chart"),
        _ => ("page", "Page"),
    };
    QuestionListRow {
        id: item.id.clone(),
        kind: kind.to_string(),
        title: if item.title.is_empty() {
            fallback.to_string()
        } else {
            item.title.clone()
        },
        tags: Vec::new(),
        time: time["label"].as_str().unwrap_or_default().to_string(),
        time_title: time["title"].as_str().unwrap_or_default().to_string(),
    }
}

/// The forms and pages whose answers are on their way but not in the transcript yet: a form whose
/// answers are sending or queued (as `unanswered` reads it), and a page or form whose answers wait
/// in the prompt queue. Their Q-tags read answered already, as the pill already skips them.
pub fn answers_on_the_way(state: &ChatState) -> Vec<PageAnswers> {
    let mut found: Vec<PageAnswers> = state
        .questions
        .forms
        .iter()
        .filter_map(|(id, form)| {
            let answers = form
                .send
                .as_ref()
                .map(|send| &send.answers)
                .or(form.sent.as_ref())?;
            Some(PageAnswers {
                page_id: id.clone(),
                title: String::new(),
                answers: answers.clone(),
            })
        })
        .collect();
    for entry in state.session.queue_prompts.iter().flatten() {
        let text = entry["prompt"]["text"]
            .as_str()
            .or_else(|| entry["text"].as_str())
            .unwrap_or_default();
        if !text.contains("<ghostex-page-answers") {
            continue;
        }
        if let Some((answers, _)) = crate::transcript::page_answers::split_page_answers(text) {
            if !found.iter().any(|known| known.page_id == answers.page_id) {
                found.push(answers);
            }
        }
    }
    found
}

/// The document's `openQuestions`.
pub fn project(state: &ChatState, context: &ChatContext) -> OpenQuestions {
    let open = unanswered(state);
    let count = open.len() as u32;
    let filter = state.questions.open_questions.filter.as_str();
    let all = list(state, context);
    let total = all.iter().map(|section| section.count).sum();
    let sections = &all;
    let in_section = |id: &str| {
        sections
            .iter()
            .find(|section| section.id == id)
            .map_or(0, |section| section.count)
    };
    let later = in_section("later");
    let summary = [
        Some(format!("{count} unanswered")),
        (later > 0).then(|| format!("{later} for later")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" \u{b7} ");
    OpenQuestions {
        count,
        text: "Previous question \u{2191}".to_string(),
        accessibility_label: match count {
            0 => format!("Chat Resources: {summary}. Show the list"),
            1 => "1 unanswered question: go to it".to_string(),
            _ => format!("{count} unanswered questions: go to the previous one"),
        },
        newest: open.last().map(|asker| asker.id.clone()),
        jump: state.questions.open_questions.jump.clone(),
        total,
        summary,
        filters: FILTERS
            .iter()
            .map(|(id, label)| QuestionListFilter {
                id: id.to_string(),
                label: label.to_string(),
                count: all
                    .iter()
                    .flat_map(|section| &section.rows)
                    .filter(|row| filter_shows(id, &row.kind))
                    .count() as u32,
            })
            .collect(),
        filter: if filter.is_empty() {
            "all".to_string()
        } else {
            filter.to_string()
        },
        sections: all
            .iter()
            .filter_map(|section| {
                let rows: Vec<QuestionListRow> = section
                    .rows
                    .iter()
                    .filter(|row| filter_shows(filter, &row.kind))
                    .cloned()
                    .collect();
                (!rows.is_empty()).then(|| QuestionListSection {
                    count: rows.len() as u32,
                    rows,
                    ..section.clone()
                })
            })
            .collect(),
    }
}

/// `previousQuestion`: the pill's click. It goes to the unanswered question before the one it went
/// to last (wrapping to the newest after the oldest), or to the newest when it has not gone
/// anywhere yet or the last one is no longer waiting.
///
/// CDXC:SessionChat 2026-10-14 WHY:
/// Starting again from the newest whenever the reader sat at the bottom was tried and dropped: the newest question usually sits at the bottom, so the second click landed on it again instead of stepping back.
pub fn previous(state: &mut ChatState, context: &ChatContext) -> Vec<Effect> {
    let open: Vec<Asker> = unanswered(state).into_iter().cloned().collect();
    if open.is_empty() {
        return Vec::new();
    }
    let last = state.questions.open_questions.last.as_deref();
    let at = open
        .iter()
        .position(|asker| Some(asker.id.as_str()) == last);
    jump_to(state, context, &open[step(open.len(), at)], None)
}

/// Which of `count` unanswered questions (oldest first) the pill goes to, given where the last
/// jump went (`at`).
fn step(count: usize, at: Option<usize>) -> usize {
    match at {
        Some(index) => index.checked_sub(1).unwrap_or(count - 1),
        None => count - 1,
    }
}

/// `jumpToQuestion {id}`: a row of the pill's list (a form or page, answered or not, or a chart).
pub fn jump_to_id(state: &mut ChatState, context: &ChatContext, id: &str) -> Vec<Effect> {
    if let Some(asker) = state
        .transcript_view
        .askers
        .iter()
        .find(|asker| asker.id == id)
        .cloned()
    {
        return jump_to(state, context, &asker, None);
    }
    let Some(item) = state
        .questions
        .content
        .items
        .iter()
        .rev()
        .find(|item| item.id == id)
        .cloned()
    else {
        return Vec::new();
    };
    jump_to_row(state, context, &item.message_id, &item.id, None)
}

/// A Q-tag link's click (`#gx-question=Q80`, `transcript/open_questions.rs`): the latest form or
/// page that asked the tag, answered or not.
pub fn jump_to_tag(state: &mut ChatState, context: &ChatContext, tag: &str) -> Vec<Effect> {
    let Some(asker) = tag_target(&state.transcript_view.askers, tag).cloned() else {
        return vec![Effect::Toast {
            level: "info".to_string(),
            message: format!("{tag} isn't asked in the loaded part of this chat."),
        }];
    };
    // The form's own spelling of the tag, which its drawn question carries.
    let question = asker
        .tags
        .iter()
        .find(|known| known.eq_ignore_ascii_case(tag))
        .cloned();
    jump_to(state, context, &asker, question)
}

fn jump_to(
    state: &mut ChatState,
    context: &ChatContext,
    asker: &Asker,
    question: Option<String>,
) -> Vec<Effect> {
    jump_to_row(state, context, &asker.message_id, &asker.id, question)
}

/// Scrolls to the row holding `message_id` and flashes `target` there.
fn jump_to_row(
    state: &mut ChatState,
    context: &ChatContext,
    message_id: &str,
    target: &str,
    question: Option<String>,
) -> Vec<Effect> {
    let rows = crate::transcript::rows(state, context);
    let Some(item) = crate::composer::files_links::row_of(&rows, message_id, None) else {
        return Vec::new();
    };
    let revision = state
        .questions
        .open_questions
        .jump
        .as_ref()
        .map_or(1, |jump| jump.revision + 1);
    let open_questions = &mut state.questions.open_questions;
    open_questions.last = Some(target.to_string());
    open_questions.jump = Some(OpenQuestionJump {
        revision,
        item,
        target: target.to_string(),
        question,
    });
    state.core.request_publish();
    Vec::new()
}

/// The tag a Markdown link names, when it is a Q-tag link (`#gx-question=Q80;open`).
pub fn link_tag(href: &str) -> Option<&str> {
    href.trim()
        .strip_prefix(crate::transcript::open_questions::QUESTION_LINK_PREFIX)
        .map(|rest| rest.split(';').next().unwrap_or_default().trim())
        .filter(|tag| !tag.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pill_steps_back_from_the_newest_and_wraps() {
        assert_eq!(step(3, None), 2, "the first click goes to the newest");
        assert_eq!(step(3, Some(2)), 1);
        assert_eq!(step(3, Some(1)), 0);
        assert_eq!(
            step(3, Some(0)),
            2,
            "after the oldest it wraps to the newest"
        );
    }

    fn asker(id: &str, kind: AskerKind, at: Option<i64>) -> Asker {
        Asker {
            id: id.to_string(),
            kind,
            message_id: format!("m-{id}"),
            tags: vec![format!("Q-{id}")],
            answered: false,
            obsolete: false,
            withdrawn: Vec::new(),
            keys: Vec::new(),
            skipped: Vec::new(),
            superseded: Vec::new(),
            inherited: false,
            title: format!("About {id}"),
            at,
        }
    }

    fn section<'a>(pill: &'a OpenQuestions, id: &str) -> Vec<&'a str> {
        pill.sections
            .iter()
            .find(|section| section.id == id)
            .map(|section| section.rows.iter().map(|row| row.id.as_str()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn the_pill_shows_while_a_question_is_unanswered() {
        let mut state = ChatState::default();
        state
            .transcript_view
            .askers
            .push(asker("form", AskerKind::Form, None));
        let pill = project(&state, &ChatContext::default());
        assert_eq!(pill.count, 1);
        assert_eq!(pill.total, 1);
        assert_eq!(pill.text, "Previous question \u{2191}");
    }

    #[test]
    fn for_later_leaves_the_count_but_stays_in_the_list() {
        let mut state = ChatState::default();
        state
            .transcript_view
            .askers
            .push(asker("a", AskerKind::Page, Some(1_000)));
        state
            .transcript_view
            .askers
            .push(asker("b", AskerKind::Form, Some(2_000)));
        state.session.later_question_ids = vec!["a".to_string()];
        let pill = project(&state, &ChatContext::default());
        assert_eq!(pill.count, 1, "only b waits");
        assert_eq!(pill.newest.as_deref(), Some("b"));
        assert_eq!(pill.summary, "1 unanswered \u{b7} 1 for later");
        assert_eq!(section(&pill, "unanswered"), vec!["b"]);
        assert_eq!(section(&pill, "later"), vec!["a"]);
        assert_eq!(later_waiting(&state), vec!["a".to_string()]);
        // Everything put off: no count, no ping, but the list stays reachable.
        state.session.later_question_ids.push("b".to_string());
        let pill = project(&state, &ChatContext::default());
        assert_eq!(pill.count, 0);
        assert_eq!(pill.newest, None);
        assert_eq!(pill.total, 2);
        // Stepping skips For later questions.
        assert!(previous(&mut state, &ChatContext::default()).is_empty());
        assert!(state.questions.open_questions.jump.is_none());
    }

    #[test]
    fn answering_a_for_later_question_moves_it_to_answered() {
        let mut state = ChatState::default();
        let mut answered = asker("a", AskerKind::Page, Some(1_000));
        answered.answered = true;
        state.transcript_view.askers.push(answered);
        state.session.later_question_ids = vec!["a".to_string()];
        let pill = project(&state, &ChatContext::default());
        assert_eq!(section(&pill, "answered"), vec!["a"]);
        assert!(section(&pill, "later").is_empty());
        assert!(
            later_waiting(&state).is_empty(),
            "its card no longer says For later"
        );
    }

    #[test]
    fn sections_come_in_order_with_their_rows_newest_first() {
        let mut state = ChatState::default();
        let mut old_answer = asker("old-answer", AskerKind::Form, Some(1_000));
        old_answer.answered = true;
        let mut new_answer = asker("new-answer", AskerKind::Page, Some(5_000));
        new_answer.answered = true;
        let mut withdrawn = asker("withdrawn", AskerKind::Form, Some(2_000));
        withdrawn.obsolete = true;
        let mut inherited = asker("inherited", AskerKind::Form, Some(500));
        inherited.inherited = true;
        let mut undated_answer = asker("undated", AskerKind::Form, None);
        undated_answer.answered = true;
        for asker in [
            inherited,
            old_answer,
            withdrawn,
            asker("open-old", AskerKind::Page, Some(3_000)),
            new_answer,
            asker("open-new", AskerKind::Form, Some(6_000)),
            undated_answer,
        ] {
            state.transcript_view.askers.push(asker);
        }
        let pill = project(&state, &ChatContext::default());
        let order: Vec<&str> = pill
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect();
        assert_eq!(
            order,
            vec!["unanswered", "answered", "withdrawn"],
            "empty sections are left out"
        );
        assert_eq!(section(&pill, "unanswered"), vec!["open-new", "open-old"]);
        assert_eq!(
            section(&pill, "answered"),
            vec!["new-answer", "old-answer", "undated"],
            "newest first; one without a time goes last"
        );
        assert_eq!(section(&pill, "withdrawn"), vec!["withdrawn"]);
        assert_eq!(
            pill.total, 6,
            "an unanswered question from before a fork is the parent's"
        );
        let answered = &pill.sections[1];
        assert_eq!(answered.title, "Answered");
        assert_eq!(answered.count, 3);
        let row = &answered.rows[0];
        assert_eq!(row.kind, "page");
        assert_eq!(row.title, "About new-answer");
        assert_eq!(row.tags, vec!["Q-new-answer".to_string()]);
        assert_eq!(row.time, "12:00 AM");
    }

    #[test]
    fn filters_pick_pages_forms_or_charts_and_count_them() {
        use crate::questions::content::ContentItem;
        use crate::transcript::question_forms::ContentKind;
        let mut state = ChatState::default();
        let mut answered_form = asker("form", AskerKind::Form, Some(2_000));
        answered_form.answered = true;
        state.transcript_view.askers.push(answered_form);
        state
            .transcript_view
            .askers
            .push(asker("question-page", AskerKind::Page, Some(3_000)));
        for (id, kind, at) in [
            ("plain-page", ContentKind::Page, 4_000),
            ("chart-1", ContentKind::Chart, 1_000),
        ] {
            state.questions.content.items.push(ContentItem {
                id: id.to_string(),
                kind,
                message_id: format!("m-{id}"),
                title: String::new(),
                at: Some(at),
            });
        }
        let pill = project(&state, &ChatContext::default());
        let counts: Vec<(&str, u32)> = pill
            .filters
            .iter()
            .map(|f| (f.label.as_str(), f.count))
            .collect();
        assert_eq!(
            counts,
            vec![("All", 4), ("Html", 2), ("Q&A", 1), ("Charts", 1)]
        );
        assert_eq!(pill.filter, "all");
        assert_eq!(
            pill.count, 1,
            "only the question page waits; pages and charts never count"
        );
        let order: Vec<&str> = pill.sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(order, vec!["unanswered", "answered", "pages", "charts"]);
        assert_eq!(pill.sections[2].rows[0].kind, "page");
        assert_eq!(
            pill.sections[2].rows[0].title, "Page",
            "a page without a title still reads as one"
        );
        assert_eq!(pill.sections[3].rows[0].kind, "chart");

        pick_filter(&mut state, "html");
        let pill = project(&state, &ChatContext::default());
        assert_eq!(pill.filter, "html");
        let html: Vec<&str> = pill
            .sections
            .iter()
            .flat_map(|s| s.rows.iter().map(|r| r.id.as_str()))
            .collect();
        assert_eq!(
            html,
            vec!["question-page", "plain-page"],
            "pages with and without questions"
        );
        assert_eq!(pill.total, 4, "the pill stays while anything is listed");

        pick_filter(&mut state, "qa");
        let pill = project(&state, &ChatContext::default());
        assert_eq!(section(&pill, "answered"), vec!["form"]);
        assert_eq!(pill.sections.len(), 1);

        pick_filter(&mut state, "bogus");
        assert_eq!(
            state.questions.open_questions.filter, "",
            "an unknown filter is All"
        );
    }

    #[test]
    fn a_question_link_names_its_tag() {
        assert_eq!(link_tag("#gx-question=Q80"), Some("Q80"));
        assert_eq!(link_tag("#gx-question=Q80;answered"), Some("Q80"));
        assert_eq!(link_tag("https://example.com"), None);
    }
}
