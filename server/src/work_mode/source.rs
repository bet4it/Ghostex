//! Where a workspace's tickets come from. Each source (Linear, GitHub issues) answers the same
//! questions: which tickets a branch or a typed reference names, which tickets the Work list shows,
//! what its caches know about one ticket, its details with comments, and how to create one. PRs,
//! checks and the Clean up offer are not a source's business: they belong to the code host
//! (GitHub) whatever the tracker is.
//!
//! CDXC:WorkMode 2026-10-27 SEE-ALSO:
//! `source_linear.rs` and `source_github.rs` (the sources); `PresentationSessionWork.tickets` and `PresentationSessionLink.state_group` in packages/gx-protocol/src/presentation.rs; the Work page's `WorkItem.source` in apps/desktop/views/work/types.ts.

use serde_json::{json, Map, Value};

use super::*;

/// A workspace's ticket source: the primary tracker (`tracker.rs`) picks one per workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WorkSourceId {
    /// Linear tickets and Linear projects.
    Linear,
    /// GitHub issues and GitHub Projects.
    Github,
}

impl WorkSourceId {
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Github => "github",
        }
    }

    /// How errors and buttons name it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Github => "GitHub",
        }
    }

    /// The list row's field for the project a ticket is in (Linear project, GitHub Project).
    pub(crate) fn project_field(self) -> &'static str {
        match self {
            Self::Linear => "linearProject",
            Self::Github => "githubProject",
        }
    }

    pub(crate) fn from_wire(value: Option<&Value>) -> Option<Self> {
        match value?.as_str()?.trim() {
            "linear" => Some(Self::Linear),
            "github" => Some(Self::Github),
            _ => None,
        }
    }
}

/// One ticket of one source: `ACM-12` of Linear, `218` of GitHub.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TicketRef {
    pub(crate) source: WorkSourceId,
    pub(crate) id: String,
}

impl TicketRef {
    pub(crate) fn new(source: WorkSourceId, id: impl Into<String>) -> Self {
        Self {
            source,
            id: id.into(),
        }
    }

    /// The key links, list rows, pins and removals are stored and matched by: `linear:ACM-12`,
    /// `issue:owner/repo#218`.
    pub(crate) fn key(&self, repo: &str) -> String {
        match self.source {
            WorkSourceId::Linear => format!("linear:{}", self.id),
            WorkSourceId::Github => format!("issue:{repo}#{}", self.id),
        }
    }

    /// What a row, a chip and the team flow show: `ACM-12`, `#218`.
    pub(crate) fn label(&self) -> String {
        match self.source {
            WorkSourceId::Linear => self.id.clone(),
            WorkSourceId::Github => format!("#{}", self.id),
        }
    }

    /// The title a row shows before the ticket was read.
    pub(crate) fn untitled(&self) -> String {
        match self.source {
            WorkSourceId::Linear => self.id.clone(),
            WorkSourceId::Github => format!("Issue #{}", self.id),
        }
    }

    /// The `source` + `ticketId` a request names; `None` when it names no ticket.
    pub(crate) fn from_params(params: &Map<String, Value>) -> Option<Result<Self, String>> {
        let id = params.get("ticketId").filter(|value| !value.is_null())?;
        let Some(source) = WorkSourceId::from_wire(params.get("source")) else {
            return Some(Err("source must be linear or github.".to_string()));
        };
        let text = id
            .as_u64()
            .map(|number| number.to_string())
            .or_else(|| id.as_str().map(str::to_string))
            .unwrap_or_default();
        Some(
            work_source(source)
                .parse_ref(&text)
                .map(|id| Self::new(source, id))
                .ok_or_else(|| match source {
                    WorkSourceId::Linear => {
                        format!("\"{text}\" is not a Linear issue ID like SPX-1245.")
                    }
                    WorkSourceId::Github => "ticketId must be a GitHub issue number.".to_string(),
                }),
        )
    }
}

/// A ticket as a source knows it. A ticket read for a session's card leaves the list's extras
/// (assignee, labels, cycle, project) empty.
#[derive(Clone, Debug)]
pub(crate) struct Ticket {
    pub(crate) ticket: TicketRef,
    pub(crate) title: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) updated_at: Option<String>,
    /// The source's own word for the state, as session links publish it: Linear's state type,
    /// GitHub's `open` or `closed`.
    pub(crate) state: Option<String>,
    /// The Work page group (`backlog`, `todo`, `progress`, `review`, `done` or `canceled`, and
    /// `closed` for a closed GitHub issue) and the source's own name for it.
    pub(crate) status: WorkItemStatus,
    pub(crate) assignee: Option<WorkItemPerson>,
    pub(crate) assigned_to_me: bool,
    pub(crate) labels: Vec<String>,
    pub(crate) cycle: Option<String>,
    pub(crate) branch_name: Option<String>,
    /// The Linear project or GitHub Project it is in.
    pub(crate) project: Option<WorkItemLink>,
    /// PRs the tracker itself ties to the ticket (Linear's GitHub attachments).
    pub(crate) pull_request_urls: Vec<String>,
    /// The team it belongs to (a Linear team key): one team's tickets are worked in one project.
    pub(crate) team: Option<String>,
}

/// What a source's caches know about one ticket.
pub(crate) enum TicketRead {
    Unread,
    /// The source says there is no such ticket.
    NotFound,
    Found(Ticket),
}

/// The tickets one source brings to the Work list, and what went wrong reading them.
#[derive(Default)]
pub(crate) struct SourceList {
    pub(crate) tickets: Vec<ListedTicket>,
    pub(crate) errors: Vec<String>,
}

pub(crate) struct ListedTicket {
    pub(crate) ticket: Ticket,
    /// The Ghostex project it is worked in, when the source knows it.
    pub(crate) project_id: Option<String>,
    /// `owner/repo` its key names (GitHub); empty for a source whose keys name no repo.
    pub(crate) repo: String,
}

/// What a source needs to answer for one project.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TicketScope<'a> {
    pub(crate) project_id: &'a str,
    /// The project's checkout, where `gh` runs.
    pub(crate) cwd: Option<&'a str>,
    /// The project's Linear key, for the calls that need the key itself.
    pub(crate) linear_api_key: Option<&'a str>,
}

/// What Create ticket sends. `extra` holds a source's own fields (Linear's team and project).
pub(crate) struct TicketDraft<'a> {
    pub(crate) title: &'a str,
    pub(crate) description: Option<&'a str>,
    pub(crate) assign_to_me: bool,
    pub(crate) extra: &'a Map<String, Value>,
}

/// A ticket a source just created.
pub(crate) struct CreatedTicket {
    pub(crate) ticket: TicketRef,
    pub(crate) url: Option<String>,
    pub(crate) branch_name: Option<String>,
    /// `owner/repo` the issue was created in (GitHub).
    pub(crate) repo: Option<String>,
}

pub(crate) trait WorkSource: Sync {
    /// `acm-12` → `ACM-12`, `#218` → `218`; `None` when the text names none of its tickets.
    fn parse_ref(&self, text: &str) -> Option<String>;
    /// Whether `read_detail` can ask at all (a Linear key; a checkout and a signed-in `gh`).
    fn can_read(&self, scope: &TicketScope) -> bool;
    /// The tickets a branch names.
    fn tickets_in_branch(&self, scope: &TicketScope, branch: &str) -> Vec<String>;
    /// The Work list's tickets from the caches the background pass filled.
    fn cached_list(&self, projects: &[WorkProjectInput], plan: &WorkFeedPlan) -> SourceList;
    /// One ticket from the caches, for a session's card and a linked ticket the list lacks.
    fn cached_ticket(&self, scope: &TicketScope, id: &str) -> TicketRead;
    /// The details page's ticket (`ticket_detail_value`). Blocking.
    fn read_detail(&self, scope: &TicketScope, id: &str, force: bool) -> Result<Value, String>;
    /// Blocking.
    fn create(&self, scope: &TicketScope, draft: &TicketDraft) -> Result<CreatedTicket, String>;
}

/// The one lookup every caller uses instead of matching on the tracker.
pub(crate) fn work_source(id: WorkSourceId) -> &'static dyn WorkSource {
    match id {
        WorkSourceId::Linear => &LinearSource,
        WorkSourceId::Github => &GithubSource,
    }
}

/// A source's ticket as `PresentationSessionLink` publishes it (`PresentationSessionWork.tickets`).
pub(crate) fn ticket_link_value(ticket: &TicketRef, repo: &str, read: &TicketRead) -> Value {
    let mut link = Map::new();
    link.insert("key".to_string(), json!(ticket.key(repo)));
    link.insert(
        "kind".to_string(),
        json!(match ticket.source {
            WorkSourceId::Linear => "linear",
            WorkSourceId::Github => "issue",
        }),
    );
    link.insert("label".to_string(), json!(ticket.label()));
    match read {
        TicketRead::Found(found) => {
            insert_text(&mut link, "title", found.title.as_deref());
            insert_text(&mut link, "state", found.state.as_deref());
            link.insert("stateName".to_string(), json!(found.status.name));
            link.insert("stateGroup".to_string(), json!(found.status.group));
            insert_text(&mut link, "url", found.url.as_deref());
            if matches!(found.status.group, "done" | "canceled" | "closed") {
                link.insert("finished".to_string(), json!(true));
            }
        }
        TicketRead::NotFound => {
            link.insert("state".to_string(), json!("notFound"));
            link.insert("stateName".to_string(), json!("Not found"));
            link.insert("finished".to_string(), json!(true));
        }
        TicketRead::Unread => {
            link.insert("loading".to_string(), json!(true));
        }
    }
    Value::Object(link)
}

/// The details page's `ticket`, one shape for every source. `description` and each comment's
/// `body` are already cut to the page's limit.
pub(crate) struct TicketDetail<'a> {
    pub(crate) ticket: &'a TicketRef,
    pub(crate) title: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) status: WorkItemStatus,
    /// The source's own state word (Linear's state type, GitHub's `open`/`closed`).
    pub(crate) state: Option<String>,
    pub(crate) fields: Map<String, Value>,
    pub(crate) comments: Vec<Value>,
    pub(crate) comment_count: usize,
}

pub(crate) fn ticket_detail_value(detail: TicketDetail) -> Value {
    let mut output = detail.fields;
    output.insert("source".to_string(), json!(detail.ticket.source.as_wire()));
    output.insert("id".to_string(), json!(detail.ticket.id));
    output.insert("label".to_string(), json!(detail.ticket.label()));
    output.insert("title".to_string(), json!(detail.title));
    output.insert("url".to_string(), json!(detail.url));
    output.insert("description".to_string(), json!(detail.description));
    output.insert(
        "state".to_string(),
        json!({ "group": detail.status.group, "name": detail.status.name, "type": detail.state }),
    );
    output.insert("comments".to_string(), Value::Array(detail.comments));
    output.insert("commentCount".to_string(), json!(detail.comment_count));
    Value::Object(output)
}

fn insert_text(output: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        output.insert(key.to_string(), json!(value));
    }
}
