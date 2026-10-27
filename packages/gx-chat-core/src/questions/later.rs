//! "For later": a question form or question page the reader put off. It stays answerable, but it
//! no longer counts as waiting: no pink dot on the session row, no count or ping on the ? pill, no
//! notification. gxserver keeps the list (`laterQuestionIds`, server/src/session_chat_open_questions.rs)
//! so every client agrees and it survives restarts; this side shows a change at once and keeps it
//! shown until the wire agrees.
//!
//! CDXC:SessionStatus 2026-10-27 DECISION:
//! User: "i wish i could just mark an inline html as "For later" so it isn't bothering me with pink dot or saying "unanswered" but then i can go back to it any time to answer it". A page card and a question form offer "For later" while they wait, and "Back to unanswered" once put off. A For-later question leaves the session's question count (the sidebar's pink dot, `answer requested (n)`), the ? pill's count, ping and the turn-end notification, and is listed under For later in the ? pill's list. Answering it moves it to Answered; gxserver then forgets the mark.
//! SEE-ALSO: server/src/session_chat_open_questions.rs (the stored list and the count), server/src/session_chat_send/answer_http.rs (`questionForLater`), questions/open_questions.rs (the pill and its list), apps/desktop/src/app/native_chat/page_card.rs and question_form.rs, apps/mobile/app/src/chat/native/transcript/PageCard.tsx and QuestionForm.tsx.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use crate::effect::Effect;
use crate::state::ChatState;
use crate::wire::{ChatRpcMethod, RpcOutcome};

/// The marks set here that the wire has not confirmed yet, by form or page id: whether it is For
/// later, and the call that set it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaterState {
    pub pending: BTreeMap<String, (bool, u64)>,
}

/// The forms and pages put off For later: gxserver's list with this client's changes on top.
pub fn later_ids(state: &ChatState) -> BTreeSet<String> {
    let mut ids: BTreeSet<String> = state.session.later_question_ids.iter().cloned().collect();
    for (id, (later, _)) in &state.questions.later.pending {
        if *later {
            ids.insert(id.clone());
        } else {
            ids.remove(id);
        }
    }
    ids
}

/// `questionForLater {id, later}`: puts a form or page off For later, or back to unanswered.
pub fn set(state: &mut ChatState, id: &str, later: bool) -> Vec<Effect> {
    let id = id.trim();
    if id.is_empty() || later_ids(state).contains(id) == later {
        return Vec::new();
    }
    let request_id = state.core.allocate_request_id();
    state
        .questions
        .later
        .pending
        .insert(id.to_string(), (later, request_id));
    state.core.request_publish();
    vec![Effect::SendRpc {
        request_id,
        method: ChatRpcMethod::AnswerSessionChatPrompt,
        params: Box::new(json!({
            "kind": "questionForLater",
            "questionId": id,
            "later": later,
            "projectId": state.identity.project_id,
            "sessionId": state.identity.session_id,
        })),
    }]
}

/// The call's answer, or `None` when it is not one of these. A refusal drops the change and says
/// why; a success keeps it shown until the wire carries it (`settle`).
pub fn rpc_settled(
    state: &mut ChatState,
    request_id: u64,
    outcome: &RpcOutcome,
) -> Option<Vec<Effect>> {
    let id = state
        .questions
        .later
        .pending
        .iter()
        .find(|(_, (_, request))| *request == request_id)
        .map(|(id, _)| id.clone())?;
    let RpcOutcome::Err { message, .. } = outcome else {
        return Some(Vec::new());
    };
    state.questions.later.pending.remove(&id);
    state.core.request_publish();
    Some(vec![Effect::Toast {
        level: "error".to_string(),
        message: format!("Couldn't change For later: {message}"),
    }])
}

/// Forgets the changes the wire now carries.
pub fn settle(state: &mut ChatState) {
    if state.questions.later.pending.is_empty() {
        return;
    }
    let wire = &state.session.later_question_ids;
    state
        .questions
        .later
        .pending
        .retain(|id, (later, _)| wire.contains(id) != *later);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_shows_at_once_and_holds_until_the_wire_agrees() {
        let mut state = ChatState::default();
        let effects = set(&mut state, "page-a", true);
        assert_eq!(effects.len(), 1, "one call to gxserver");
        assert!(later_ids(&state).contains("page-a"));
        // A frame from before the write still says nothing: the mark stays.
        settle(&mut state);
        assert!(later_ids(&state).contains("page-a"));
        state.session.later_question_ids = vec!["page-a".to_string()];
        settle(&mut state);
        assert!(state.questions.later.pending.is_empty());
        assert!(later_ids(&state).contains("page-a"));
        // Back to unanswered.
        set(&mut state, "page-a", false);
        assert!(!later_ids(&state).contains("page-a"));
        assert!(
            set(&mut state, "page-a", false).is_empty(),
            "already unanswered"
        );
    }

    #[test]
    fn a_refused_mark_is_dropped() {
        let mut state = ChatState::default();
        set(&mut state, "form-a", true);
        let request = state.questions.later.pending["form-a"].1;
        let effects = rpc_settled(
            &mut state,
            request,
            &RpcOutcome::Err {
                code: None,
                message: "no".to_string(),
                endpoint: None,
            },
        );
        assert!(matches!(effects.as_deref(), Some([Effect::Toast { .. }])));
        assert!(!later_ids(&state).contains("form-a"));
    }
}
