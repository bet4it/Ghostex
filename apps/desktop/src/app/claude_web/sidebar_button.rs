//! The Claude button in the sidebar's top row (drawn by native_sidebar/navigation.rs): it
//! opens the Claude view over the whole work area and takes you back where you were.
//!
//! CDXC:Extensions 2026-10-28 DECISION:
//! User: the built-in Claude extension, when enabled, "adds a button to the bottom right of the sidebar" (2026-10-25), and clicking it "just switches to a global page that just keeps just my sidebar shown". Then: "move the Claude button here up to where 'Issues' button is and make this button match this color #7C828C (shouldn't be orange)". The button now sits in the sidebar's top row beside Issues (whether or not Issues shows), drawn in the row's muted grey (#7C828C in the dark theme, #6B7280 in the light one, the same as its neighbours), and it opens the view expanded (the sessions column folded away, the sidebar left docked), and the same button goes back to the view and layout that were there before; clicking a session in the sidebar while the page is up closes it and shows the session (the user's answer to "what happens when you click a session", 2026-10-25). A view tab the button added leaves the tab strip again on the way back; its page stays loaded until the view's ordinary sleep closes it, so the next click shows it at once.
//! SEE-ALSO: native_sidebar/sessions.rs (the session click).

use std::cell::RefCell;

use crate::app::native_sidebar::navigation::SidebarExtensionButton;
use crate::*;

/// What the button's page replaced, put back on the way out.
struct SidebarExtensionTakeover {
    id: ExtensionId,
    /// The project it replaced it in: each project keeps its own view tabs.
    project_id: Option<String>,
    mode: TitlebarMode,
    maximized: bool,
    picker_open: bool,
    tab_was_open: bool,
}

thread_local! {
    static TAKEOVER: RefCell<Option<SidebarExtensionTakeover>> = const { RefCell::new(None) };
}

impl GhostexGpuiApp {
    /// The sidebar's top-row buttons: Claude's, while Claude is on and its view can open here.
    pub(crate) fn sidebar_extension_buttons(&self) -> Vec<SidebarExtensionButton> {
        let mode = TitlebarMode::Extension(super::claude_web_view_id());
        let showing = self.active_mode == mode;
        // A page closed another way (its tab, turning Claude off) has nothing left to put back: a
        // later session click must not restore that old layout over a Claude view opened from
        // its tab.
        if !showing {
            TAKEOVER.with(|current| current.borrow_mut().take());
        }
        if !super::claude_web_enabled() || !self.titlebar_mode_available(mode) {
            return Vec::new();
        }
        vec![SidebarExtensionButton {
            id: CLAUDE_WEB_VIEW_ID.to_string(),
            title: "Claude".to_string(),
            icon: "agent-icons/claude.svg",
            showing,
        }]
    }

    /// The button: opens the extension's view expanded beside the sidebar, or goes back.
    pub(crate) fn toggle_sidebar_extension_view(
        &mut self,
        extension_id: &str,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let Some(id) = ExtensionId::new(extension_id) else {
            return;
        };
        let mode = TitlebarMode::Extension(id);
        if self.active_mode == mode {
            // Opened another way (its tab, the view picker): the button closes it like the tab's
            // close button does.
            if !self.close_sidebar_extension_view(window, cx) {
                self.close_view_tab(mode, window, cx);
            }
            return;
        }
        let takeover = SidebarExtensionTakeover {
            id,
            project_id: self.agents_workspace_project_id.clone(),
            mode: self.active_mode,
            maximized: self.view_panel_maximized,
            picker_open: self.view_panel_picker_open,
            tab_was_open: self.open_views.contains(&mode),
        };
        let sidebar_docked = !self.sidebar_collapsed;
        if !self.open_view_tab(mode, window, cx) {
            self.dispatch_gpui_workspace_action_toast(
                "warning",
                &format!("{} could not open", mode.tab_label()),
                "Open a project in this window first.",
                cx,
            );
            return;
        }
        if !self.view_panel_maximized {
            self.toggle_view_panel_maximized(cx);
        }
        // A sidebar remembered per view could fold away with the switch; this page keeps it.
        if sidebar_docked && self.sidebar_collapsed {
            self.toggle_gpui_sidebar_collapsed(cx);
        }
        TAKEOVER.with(|current| *current.borrow_mut() = Some(takeover));
        cx.notify();
    }

    /// Puts back what the button's page replaced, when that page is the one on screen. Returns
    /// whether it did.
    pub(crate) fn close_sidebar_extension_view(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let Some(takeover) = TAKEOVER.with(|current| current.borrow_mut().take()) else {
            return false;
        };
        let mode = TitlebarMode::Extension(takeover.id);
        if self.active_mode != mode || self.agents_workspace_project_id != takeover.project_id {
            return false;
        }
        if !takeover.tab_was_open {
            self.open_views.retain(|tab| *tab != mode);
        }
        let back = if takeover.mode == TitlebarMode::Agents
            || (self.open_views.contains(&takeover.mode)
                && self.titlebar_mode_available(takeover.mode))
        {
            takeover.mode
        } else {
            TitlebarMode::Agents
        };
        self.view_panel_picker_open = takeover.picker_open && back == TitlebarMode::Agents;
        self.view_panel_maximized = takeover.maximized;
        self.set_active_mode(back, window, cx);
        // The sessions column comes back with the old layout, so its terminals and chats are
        // reconciled the way Expand's own toggle does.
        self.reconcile_agents_pane_surfaces(cx);
        self.update_active_mode_cef_child_visibility(cx);
        self.persist_shell_layout_state();
        cx.notify();
        true
    }
}
