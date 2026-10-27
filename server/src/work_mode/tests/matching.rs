//! How a branch, a PR body and a request name a ticket.

use serde_json::{json, Map, Value};

use super::*;

fn params(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

#[test]
fn a_branch_names_linear_tickets_only_by_real_team_keys() {
    let teams = vec!["ACM".to_string(), "Web".to_string()];
    assert_eq!(
        linear_identifiers_in_branch("leandro/acm-12-and-web-3_ACM-12", &teams),
        vec!["ACM-12".to_string(), "WEB-3".to_string()]
    );
    assert!(linear_identifiers_in_branch("fix-utf-8-paste", &teams).is_empty());
    assert!(linear_identifiers_in_branch("spx-1245-x", &teams).is_empty());
}

#[test]
fn a_branch_names_a_github_issue_by_its_last_segment() {
    assert_eq!(
        github_issue_in_branch("yahia/218-arabic-plan-cards"),
        Some(218)
    );
    assert_eq!(github_issue_in_branch("218-x"), Some(218));
    assert_eq!(github_issue_in_branch("218/arabic"), None);
    assert_eq!(github_issue_in_branch("v2-218"), None);
    assert_eq!(github_issue_in_branch("0-zero"), None);
    assert_eq!(github_issue_in_branch("1234"), None);
}

#[test]
fn a_pr_body_names_the_issues_it_closes() {
    assert_eq!(
        closing_issue_numbers("Fixes #6, closes #9 and resolves #12. Refs #40."),
        vec![9, 6, 12]
    );
}

#[test]
fn linear_identifiers_are_normalized() {
    assert_eq!(
        normalize_linear_identifier(" spx-1245 "),
        Some("SPX-1245".to_string())
    );
    assert_eq!(normalize_linear_identifier("1ab-2"), None);
    assert_eq!(normalize_linear_identifier("ABC-x"), None);
    assert_eq!(normalize_linear_identifier("ABCDEFGHIJK-1"), None);
}

#[test]
fn a_request_names_a_linear_issue_a_github_issue_or_a_pr() {
    let cases = [
        (
            json!({ "linearIssue": "acm-1" }),
            Some(WorkItemRef::Linear("ACM-1".to_string())),
        ),
        (
            json!({ "githubIssue": 218 }),
            Some(WorkItemRef::GithubIssue(218)),
        ),
        (
            json!({ "githubIssue": "#218" }),
            Some(WorkItemRef::GithubIssue(218)),
        ),
        (
            json!({ "pullRequest": 41 }),
            Some(WorkItemRef::PullRequest("41".to_string())),
        ),
        (
            json!({ "pullRequest": " #41 " }),
            Some(WorkItemRef::PullRequest("41".to_string())),
        ),
        (
            json!({ "pullRequest": "https://github.com/acme/app/pull/41" }),
            Some(WorkItemRef::PullRequest(
                "https://github.com/acme/app/pull/41".to_string(),
            )),
        ),
        // The Linear issue wins when a request names several.
        (
            json!({ "linearIssue": "ACM-1", "githubIssue": 2, "pullRequest": 3 }),
            Some(WorkItemRef::Linear("ACM-1".to_string())),
        ),
        (
            json!({ "linearIssue": "not a ticket", "githubIssue": 2 }),
            Some(WorkItemRef::GithubIssue(2)),
        ),
        (json!({ "pullRequest": "" }), None),
        (json!({}), None),
    ];
    for (request, expected) in cases {
        assert_eq!(
            WorkItemRef::from_params(&params(request.clone())),
            expected,
            "{request}"
        );
    }
}

#[test]
fn linear_states_map_to_the_work_page_groups() {
    let cases = [
        (Some("started"), Some("In Review"), "review", "In Review"),
        (
            Some("started"),
            Some("In Progress"),
            "progress",
            "In Progress",
        ),
        (Some("unstarted"), Some("Todo"), "todo", "Todo"),
        (Some("backlog"), Some("Backlog"), "backlog", "Backlog"),
        (Some("triage"), Some("Triage"), "backlog", "Triage"),
        (Some("completed"), Some("Done"), "done", "Done"),
        (Some("canceled"), Some("Canceled"), "canceled", "Canceled"),
        (None, None, "todo", "Open"),
    ];
    for (state_type, state_name, group, name) in cases {
        let status = linear_status(state_type, state_name);
        assert_eq!((status.group, status.name.as_str()), (group, name));
    }
}
