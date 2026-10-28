//! Turning Claude on and off: Claude takes the sessions column's place and the panels around it
//! step aside, then come back.
//!
//! CDXC:Extensions 2026-10-28 DECISION:
//! User: "when i toggle claude on then please make it take over the whole area (so the sidebar is hidden basically in this case). no need to show a tab for it on the right sidepanel. i should still be able to open stuff on the right sidebar like browser etc while claude is open (but by default it hides right sidepanel and left sidebar and bottom commands pane when i toggle it on pls". Claude is never a side panel tab: its page is drawn where the sessions column is (claude_web/column.rs), turning it on hides the sidebar, the side panel and the Commands pane, and the side panel, the sidebar and the Commands pane still open beside it with their usual toggles and shortcuts. Opening the Claude view any other way (the Open a view picker, a saved tab from an older build) turns Claude on. This supersedes the 2026-10-25 rule that Claude's page kept the sidebar shown and opened as an expanded side panel view (claude_web/sidebar_button.rs, packages/settings-catalog/src/data/official_extensions.rs).
//! SEE-ALSO: render/workarea_split.rs and render/command_pane_structure.rs (the column), workarea/runtime_surfaces.rs, keyboard_owner.rs, focus/shell_focus.rs and tab_actions/focused_surface.rs (`view_page_on_screen`), workspace_reconcile/active_mode.rs (opening the view turns Claude on).

use gpui::Window;
use serde_json::{Value, json};

use super::view::{claude_web_enabled, claude_web_view_id};
use crate::*;

/// The side panel as Claude found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClaudeSidePanel {
    /// The view it showed; `Agents` is the Open a view picker.
    pub(crate) mode: TitlebarMode,
    pub(crate) maximized: bool,
}

/// Which of the panels Claude hides are on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClaudeLayout {
    pub(crate) sidebar: bool,
    pub(crate) side_panel: Option<ClaudeSidePanel>,
    pub(crate) commands: bool,
}

impl ClaudeLayout {
    /// What turning Claude on leaves: Claude alone in the window.
    pub(crate) const CLAUDE_ALONE: Self = Self {
        sidebar: false,
        side_panel: None,
        commands: false,
    };

    /// What turning Claude off puts on screen, from the layout Claude found (`before`) and the one
    /// on screen now.
    ///
    /// CDXC:Extensions 2026-10-28 WHY:
    /// Every panel comes back as it was before Claude, except one that is on screen when Claude turns off, which stays as it is (a side panel keeps the view it shows now). The user opened it on purpose while Claude was on, so taking it away would undo what they just did; a panel that is hidden is indistinguishable from Claude's own hiding, which was only ever meant to last while Claude is on, so it goes back.
    pub(crate) fn after_claude(before: Self, now: Self) -> Self {
        Self {
            sidebar: before.sidebar || now.sidebar,
            side_panel: now.side_panel.or(before.side_panel),
            commands: before.commands || now.commands,
        }
    }

    fn to_shell_state_json(self) -> Value {
        json!({
            "sidebar": self.sidebar,
            "sidePanel": self.side_panel.map(|panel| json!({
                "mode": panel.mode.element_slug(),
                "maximized": panel.maximized,
            })),
            "commands": self.commands,
        })
    }

    fn from_shell_state_json(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let side_panel = match object.get("sidePanel") {
            Some(Value::Object(panel)) => Some(ClaudeSidePanel {
                mode: panel
                    .get("mode")
                    .and_then(Value::as_str)
                    .and_then(TitlebarMode::from_slug)
                    .filter(|mode| !mode.is_claude_web())?,
                maximized: panel
                    .get("maximized")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }),
            _ => None,
        };
        Some(Self {
            sidebar: object.get("sidebar")?.as_bool()?,
            side_panel,
            commands: object.get("commands")?.as_bool()?,
        })
    }
}

/// Claude is on: the window's layout from before, to put back when it goes off. Saved with the
/// window's shell state, so a relaunch comes back with Claude on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClaudeTakeover {
    before: ClaudeLayout,
}

impl ClaudeTakeover {
    /// Turning Claude on over `current`: the takeover to keep, and the layout to show.
    pub(crate) fn turn_on(current: ClaudeLayout) -> (Self, ClaudeLayout) {
        (Self { before: current }, ClaudeLayout::CLAUDE_ALONE)
    }

    /// Turning Claude off over `current`: the layout to show.
    pub(crate) fn turn_off(self, current: ClaudeLayout) -> ClaudeLayout {
        ClaudeLayout::after_claude(self.before, current)
    }

    pub(crate) fn to_shell_state_json(self) -> Value {
        self.before.to_shell_state_json()
    }

    pub(crate) fn from_shell_state_json(value: &Value) -> Option<Self> {
        ClaudeLayout::from_shell_state_json(value).map(|before| Self { before })
    }
}

impl GhostexGpuiApp {
    pub(crate) fn claude_on(&self) -> bool {
        self.claude_takeover.is_some()
    }

    /// Claude's page is on screen: Claude is on and no expanded side panel covers its column.
    pub(crate) fn claude_page_on_screen(&self) -> bool {
        self.claude_on() && !self.view_panel_maximized()
    }

    /// Whether a view's page is on screen: the side panel's view, or Claude in the sessions
    /// column's place. Visibility, keyboard and focus ask this instead of comparing `active_mode`,
    /// which only ever names the side panel's view.
    pub(crate) fn view_page_on_screen(&self, mode: TitlebarMode) -> bool {
        self.active_mode == mode || (mode.is_claude_web() && self.claude_page_on_screen())
    }

    fn claude_layout_now(&self) -> ClaudeLayout {
        ClaudeLayout {
            sidebar: !self.sidebar_collapsed,
            side_panel: self.view_panel_open().then_some(ClaudeSidePanel {
                mode: self.active_mode,
                maximized: self.view_panel_maximized,
            }),
            commands: self.command_pane.is_expanded(),
        }
    }

    /// Shows `layout`'s panels. A side panel view that is no longer open or available stays closed.
    fn apply_claude_layout(&mut self, layout: ClaudeLayout, cx: &mut gpui::Context<Self>) {
        match layout.side_panel {
            None if self.view_panel_open() => self.close_view_panel(cx),
            None => {}
            Some(panel) => {
                if !self.view_panel_open() {
                    // Set first, so the panel opens expanded instead of expanding after.
                    self.view_panel_maximized = panel.maximized;
                    if panel.mode == TitlebarMode::Agents {
                        self.open_view_picker(cx);
                    } else if self.open_views.contains(&panel.mode) {
                        self.activate_mode(panel.mode, cx);
                    }
                    if !self.view_panel_open() {
                        self.view_panel_maximized = false;
                    }
                }
                if self.view_panel_open() && self.view_panel_maximized != panel.maximized {
                    self.toggle_view_panel_maximized(cx);
                }
            }
        }
        // After the side panel: a sidebar remembered per view moves with the view.
        if self.sidebar_collapsed == layout.sidebar {
            self.toggle_gpui_sidebar_collapsed(cx);
        }
        if self.command_pane.is_expanded() != layout.commands {
            self.command_pane_auto_minimize.idle_since = None;
            if layout.commands {
                self.command_pane.expand();
            } else {
                self.clear_command_pane_keep_open();
                self.command_pane.collapse();
                self.clear_command_resize_hover_state();
            }
            self.refresh_sidebar_command_pane_sessions_if_changed(cx);
        }
    }

    /// The Claude button: turns Claude on, or off when it is on.
    pub(crate) fn toggle_claude(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.claude_on() {
            self.turn_claude_off(cx);
            self.sync_browser_address_input_if_shown(window, cx);
        } else if !self.show_claude(cx) {
            self.dispatch_gpui_workspace_action_toast(
                "warning",
                "Claude could not open",
                "Open a project in this window first.",
                cx,
            );
        }
    }

    /// Turns Claude on, or with Claude on wakes and focuses its page. Returns whether Claude is on.
    pub(crate) fn show_claude(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        let id = claude_web_view_id();
        let mode = TitlebarMode::Extension(id);
        if !claude_web_enabled() || !self.titlebar_mode_available(mode) {
            return false;
        }
        if self.claude_takeover.is_none() {
            // A Claude tab saved by an older build leaves the side panel, and so does the Open a
            // view picker, which Claude may just have been picked from: neither comes back after.
            if self.active_mode == mode || self.view_picker_open() {
                self.close_view_panel(cx);
            }
            self.open_views.retain(|tab| *tab != mode);
            let (takeover, alone) = ClaudeTakeover::turn_on(self.claude_layout_now());
            self.claude_takeover = Some(takeover);
            self.apply_claude_layout(alone, cx);
        } else if self.view_picker_open() {
            // Picked in the side panel's Open a view picker: the panel steps aside for Claude.
            self.close_view_panel(cx);
        } else if self.view_panel_maximized() {
            // An expanded side panel covers Claude's column.
            self.toggle_view_panel_maximized(cx);
        }
        self.mark_project_editor_mode_awake(mode, cx);
        self.ensure_extension_view_runtime_for_current_context(id, cx);
        self.ensure_project_workarea_runtime_cef_surfaces_for_current_context(cx);
        self.focus_shell_target(ShellFocusTarget::ProjectEditorSurface(mode), cx);
        // The sessions column just left the screen, so its terminals and chats hide.
        self.update_active_mode_cef_child_visibility(cx);
        self.persist_shell_layout_state();
        cx.notify();
        true
    }

    /// Turns Claude off and puts the layout back (`ClaudeLayout::after_claude`). Its page stays
    /// loaded until the awake cap puts it to sleep, so turning Claude on again shows it at once.
    /// Returns whether Claude was on.
    pub(crate) fn turn_claude_off(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        let Some(takeover) = self.claude_takeover.take() else {
            return false;
        };
        let after = takeover.turn_off(self.claude_layout_now());
        self.apply_claude_layout(after, cx);
        self.focus_default_surface_for_active_mode(cx);
        self.reconcile_agents_pane_surfaces(cx);
        self.update_active_mode_cef_child_visibility(cx);
        self.persist_shell_layout_state();
        cx.notify();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BROWSER: ClaudeSidePanel = ClaudeSidePanel {
        mode: TitlebarMode::Browser,
        maximized: false,
    };
    const KANBAN: ClaudeSidePanel = ClaudeSidePanel {
        mode: TitlebarMode::Kanban,
        maximized: true,
    };
    const EVERYTHING: ClaudeLayout = ClaudeLayout {
        sidebar: true,
        side_panel: Some(KANBAN),
        commands: true,
    };

    #[test]
    fn turning_claude_on_hides_the_sidebar_the_side_panel_and_the_commands_pane() {
        let (_, shown) = ClaudeTakeover::turn_on(EVERYTHING);
        assert_eq!(shown, ClaudeLayout::CLAUDE_ALONE);
        assert!(!shown.sidebar && shown.side_panel.is_none() && !shown.commands);
    }

    #[test]
    fn turning_claude_off_brings_back_the_layout_from_before() {
        let (takeover, shown) = ClaudeTakeover::turn_on(EVERYTHING);
        assert_eq!(takeover.turn_off(shown), EVERYTHING);

        let bare = ClaudeLayout {
            sidebar: false,
            side_panel: None,
            commands: false,
        };
        let (takeover, shown) = ClaudeTakeover::turn_on(bare);
        assert_eq!(takeover.turn_off(shown), bare);
    }

    #[test]
    fn a_panel_opened_while_claude_is_on_stays_open_when_it_turns_off() {
        let before = ClaudeLayout {
            sidebar: true,
            side_panel: None,
            commands: false,
        };
        let (takeover, shown) = ClaudeTakeover::turn_on(before);
        // The user opens the Browser in the side panel and the Commands pane beside Claude.
        let now = ClaudeLayout {
            side_panel: Some(BROWSER),
            commands: true,
            ..shown
        };
        assert_eq!(
            takeover.turn_off(now),
            ClaudeLayout {
                sidebar: true,
                side_panel: Some(BROWSER),
                commands: true,
            }
        );
    }

    #[test]
    fn the_side_panel_opened_while_claude_is_on_keeps_its_view_over_the_one_from_before() {
        let (takeover, shown) = ClaudeTakeover::turn_on(EVERYTHING);
        let now = ClaudeLayout {
            side_panel: Some(BROWSER),
            ..shown
        };
        assert_eq!(takeover.turn_off(now).side_panel, Some(BROWSER));
    }

    #[test]
    fn a_panel_opened_and_closed_again_while_claude_is_on_goes_back_to_how_it_was() {
        let (takeover, shown) = ClaudeTakeover::turn_on(EVERYTHING);
        // Opened and closed again: hidden, as Claude left it.
        assert_eq!(takeover.turn_off(shown), EVERYTHING);
    }

    #[test]
    fn turning_claude_on_and_off_again_starts_from_the_layout_the_last_turn_left() {
        let before = ClaudeLayout {
            sidebar: true,
            side_panel: None,
            commands: false,
        };
        let (takeover, shown) = ClaudeTakeover::turn_on(before);
        let after_first = takeover.turn_off(ClaudeLayout {
            side_panel: Some(BROWSER),
            ..shown
        });

        let (takeover, shown) = ClaudeTakeover::turn_on(after_first);
        assert_eq!(shown, ClaudeLayout::CLAUDE_ALONE);
        assert_eq!(takeover.turn_off(shown), after_first);
        assert_eq!(after_first.side_panel, Some(BROWSER));
    }

    #[test]
    fn the_layout_from_before_survives_a_relaunch() {
        let (takeover, _) = ClaudeTakeover::turn_on(EVERYTHING);
        let saved = takeover.to_shell_state_json();
        assert_eq!(
            ClaudeTakeover::from_shell_state_json(&saved),
            Some(takeover)
        );

        let picker = ClaudeLayout {
            sidebar: false,
            side_panel: Some(ClaudeSidePanel {
                mode: TitlebarMode::Agents,
                maximized: false,
            }),
            commands: true,
        };
        let (takeover, _) = ClaudeTakeover::turn_on(picker);
        assert_eq!(
            ClaudeTakeover::from_shell_state_json(&takeover.to_shell_state_json()),
            Some(takeover)
        );
        assert_eq!(ClaudeTakeover::from_shell_state_json(&Value::Null), None);
    }
}
