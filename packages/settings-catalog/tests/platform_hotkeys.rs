//! Pins what every default hotkey binds to on each platform.
//!
//! macOS must keep exactly the bindings it had before issue #315 moved the Windows and Linux
//! defaults off the plain Ctrl keys a terminal needs (CDXC:Hotkeys 2026-10-26 DECISION in
//! src/hotkey_definitions.rs): the chord of every hotkey, and the app action each formerly
//! hard-coded key runs (apps/desktop/src/app/shell_hotkeys.rs, read from source because the
//! desktop crate is too heavy to build for a test). Windows and Linux must follow Ghostty's
//! Linux defaults (.dependencies/ghostty/src/config/Config.zig, `Keybinds.init`).

use std::collections::BTreeMap;

use ghostex_settings_catalog::{
    hotkey_definitions, hotkey_keystroke, platform_hotkey, Platform, VIEW_SCOPED_HOTKEY_IDS,
};

/// The GPUI keystroke each default binds on `platform` (`""` when it has none there).
fn default_keystrokes(platform: Platform) -> BTreeMap<&'static str, String> {
    hotkey_definitions()
        .iter()
        .map(|definition| {
            let key = platform_hotkey(definition.id, definition.default_key, platform);
            (
                definition.id,
                hotkey_keystroke(key, platform).unwrap_or_default(),
            )
        })
        .collect()
}

#[test]
fn macos_hotkeys_keep_their_chords() {
    let pinned: BTreeMap<&str, String> = [
        ("scrollChatToBottom", "ctrl-shift-down"),
        ("focusChatComposer", ""),
        ("copyLastChatCodeBlock", "cmd-shift-;"),
        ("copyLastChatReply", "cmd-shift-c"),
        ("toggleChatSummaryMode", "ctrl-alt-s"),
        ("toggleChatComposerMaximized", "ctrl-alt-m"),
        ("toggleChatFilesLinks", "ctrl-alt-l"),
        ("toggleVoiceAgent", "ctrl-alt-v"),
        ("voiceCallAboutSession", ""),
        ("holdToTalk", ""),
        ("toggleDictation", "ctrl-alt-d"),
        ("readAloudLastReply", ""),
        ("readAloudStop", ""),
        ("createAgentSession", "cmd-shift-o"),
        ("createSession", "cmd-shift-t"),
        ("openCommandPalette", "cmd-shift-p"),
        ("openSessionSearchPalette", "cmd-p"),
        ("openProjectSearchPalette", "cmd-alt-shift-o"),
        ("openQuickAccessFiles", "cmd-o"),
        ("openNewThreadPalette", "cmd-n"),
        ("openCommandsPanel", "cmd-j"),
        ("openCommandsPanelSecondKey", "shift-escape"),
        ("openSettings", "cmd-,"),
        ("openExtensions", ""),
        ("openGhostexHelp", ""),
        ("openFileInFiles", ""),
        ("openHotkeys", "cmd-/"),
        ("toggleSidebarCollapsed", "cmd-b"),
        ("toggleViewPanel", "cmd-alt-b"),
        ("expandViewPanel", "cmd-ctrl-e"),
        ("expandViewPanelFully", "cmd-ctrl-shift-e"),
        ("renameActiveSession", "cmd-r"),
        ("openBrowserPane", "cmd-t"),
        ("switchAgentsView", ""),
        ("switchSourceView", ""),
        ("switchGitHubView", ""),
        ("switchKanbanView", ""),
        ("switchManageView", ""),
        ("switchAutomateView", ""),
        ("switchChangesView", ""),
        ("switchTerminalView", ""),
        ("switchTitlebarView1", "alt-1"),
        ("switchTitlebarView2", "alt-2"),
        ("switchTitlebarView3", "alt-3"),
        ("switchTitlebarView4", "alt-4"),
        ("switchTitlebarView5", "alt-5"),
        ("switchTitlebarView6", "alt-6"),
        ("switchTitlebarView7", "alt-7"),
        ("switchTitlebarView8", "alt-8"),
        ("switchTitlebarView9", "alt-9"),
        ("rotatePanesClockwise", "ctrl-shift-l"),
        ("mergeAllTabs", "ctrl-shift-m"),
        ("delayedSend", "ctrl-shift-s"),
        ("closeAfterDone", ""),
        ("openModelPicker", "alt-p"),
        ("promptEditor", "ctrl-g"),
        ("attachFileOrFolder", "cmd-alt-p"),
        ("sessionNote", "cmd-alt-n"),
        ("stashPrompt", "alt-s"),
        ("stashedPrompts", "cmd-alt-s"),
        ("exportTranscript", "cmd-alt-e"),
        ("toggleAgentActions", "cmd-alt-a"),
        ("toggleChatView", "alt-g"),
        ("openFindPrompts", "cmd-shift-f"),
        ("searchProjectFiles", "cmd-shift-f"),
        ("scrollTerminalToTop", ""),
        ("scrollTerminalToBottom", ""),
        ("splitSessionRight", "alt-shift-d"),
        ("forkSession", "cmd-ctrl-shift-f"),
        ("reloadSession", ""),
        ("sleepFocusedSession", "cmd-shift-a"),
        ("wakeFocusedSession", ""),
        ("closeFocusedSession", "cmd-shift-backspace"),
        ("popOutPane", "ctrl-shift-o"),
        ("focusPreviousGroup", ""),
        ("focusNextGroup", ""),
        ("navigateHistoryBack", "cmd-["),
        ("navigateHistoryForward", "cmd-]"),
        ("openNotifications", "cmd-i"),
        ("jumpToLatestUnreadNotification", "cmd-shift-u"),
        ("deferNotificationAndJumpNext", "cmd-ctrl-u"),
        ("focusPreviousSession", "ctrl-shift-tab"),
        ("focusNextSession", "ctrl-tab"),
        ("focusPreviousPaneTab", "cmd-alt-["),
        ("focusNextPaneTab", "cmd-alt-]"),
        ("focusUp", "cmd-alt-up"),
        ("focusRight", "cmd-alt-right"),
        ("focusDown", "cmd-alt-down"),
        ("focusLeft", "cmd-alt-left"),
        ("jumpToProject1", "cmd-ctrl-1"),
        ("jumpToProject2", "cmd-ctrl-2"),
        ("jumpToProject3", "cmd-ctrl-3"),
        ("jumpToProject4", "cmd-ctrl-4"),
        ("jumpToProject5", "cmd-ctrl-5"),
        ("jumpToProject6", "cmd-ctrl-6"),
        ("jumpToProject7", "cmd-ctrl-7"),
        ("jumpToProject8", "cmd-ctrl-8"),
        ("jumpToProject9", "cmd-ctrl-9"),
        ("goToSpace1", "cmd-alt-shift-1"),
        ("goToSpace2", "cmd-alt-shift-2"),
        ("goToSpace3", "cmd-alt-shift-3"),
        ("goToSpace4", "cmd-alt-shift-4"),
        ("goToSpace5", "cmd-alt-shift-5"),
        ("goToSpace6", "cmd-alt-shift-6"),
        ("goToSpace7", "cmd-alt-shift-7"),
        ("goToSpace8", "cmd-alt-shift-8"),
        ("goToSpace9", "cmd-alt-shift-9"),
        ("focusSessionSlot1", "cmd-1"),
        ("focusSessionSlot2", "cmd-2"),
        ("focusSessionSlot3", "cmd-3"),
        ("focusSessionSlot4", "cmd-4"),
        ("focusSessionSlot5", "cmd-5"),
        ("focusSessionSlot6", "cmd-6"),
        ("focusSessionSlot7", "cmd-7"),
        ("focusSessionSlot8", "cmd-8"),
        ("focusSessionSlot9", "cmd-9"),
        ("runActionSlot1", "ctrl-shift-1"),
        ("runActionSlot2", "ctrl-shift-2"),
        ("runActionSlot3", "ctrl-shift-3"),
        ("runActionSlot4", "ctrl-shift-4"),
        ("runActionSlot5", "ctrl-shift-5"),
        ("splitMore", "cmd-d"),
        ("splitMoreDown", "cmd-shift-d"),
        // Hard-coded in apps/desktop/src/main.rs until 2026-10-26, at these chords.
        ("pasteIntoTerminal", "cmd-v"),
        ("findInFocusedView", "cmd-f"),
        ("findNextInBrowser", "cmd-g"),
        ("findPreviousInBrowser", "cmd-shift-g"),
        ("zoomInFocusedView", "cmd-="),
        ("zoomOutFocusedView", "cmd--"),
        ("resetFocusedViewZoom", "cmd-0"),
        ("closeFocusedSurface", "cmd-w"),
        ("openNewWindow", "cmd-shift-n"),
        ("toggleAgentsFocusMode", "cmd-ctrl-f"),
    ]
    .into_iter()
    .map(|(id, keystroke)| (id, keystroke.to_string()))
    .collect();
    assert_eq!(default_keystrokes(Platform::MacOs), pinned);
}

/// `SHELL_HOTKEY_ACTIONS` in apps/desktop/src/app/shell_hotkeys.rs: hotkey id to the app action
/// it binds.
fn desktop_shell_actions() -> Vec<(String, String)> {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/desktop/src/app/shell_hotkeys.rs"
    ))
    .expect("apps/desktop/src/app/shell_hotkeys.rs");
    let start = source
        .find("const SHELL_HOTKEY_ACTIONS")
        .expect("SHELL_HOTKEY_ACTIONS");
    let end = start + source[start..].find("];").expect("end of the table");
    // One `("id", || Box::new(Action))` entry per split, however rustfmt wraps it.
    source[start..end]
        .split("(\"")
        .skip(1)
        .filter_map(|entry| {
            let id = entry.split('"').next()?;
            let action = entry.split("Box::new(").nth(1)?.split(')').next()?;
            Some((id.to_string(), action.to_string()))
        })
        .collect()
}

#[test]
fn macos_hotkeys_keep_their_app_actions() {
    // The keys apps/desktop/src/main.rs bound to these actions before 2026-10-26.
    let expected = [
        ("pasteIntoTerminal", "PasteIntoFocusedTerminal"),
        ("findInFocusedView", "FindInFocusedTerminal"),
        ("findNextInBrowser", "FindNextInFocusedBrowser"),
        ("findPreviousInBrowser", "FindPreviousInFocusedBrowser"),
        ("zoomInFocusedView", "ZoomInFocusedSurface"),
        ("zoomOutFocusedView", "ZoomOutFocusedSurface"),
        ("resetFocusedViewZoom", "ResetFocusedSurfaceZoom"),
        ("closeFocusedSurface", "CloseFocusedSurface"),
        ("toggleSidebarCollapsed", "ToggleGpuiSidebarCollapsed"),
        ("toggleViewPanel", "ToggleViewPanel"),
        ("createSession", "NewTerminalTab"),
        ("openNewWindow", "NewGhostexGpuiWindow"),
        ("splitMore", "SplitFocusedTerminalRight"),
        ("splitMoreDown", "SplitFocusedTerminalDown"),
        ("openBrowserPane", "NewBrowserTab"),
        ("toggleAgentsFocusMode", "ToggleAgentsFocusMode"),
        ("mergeAllTabs", "MergeAllTabs"),
        ("focusLeft", "FocusWorkspaceLeft"),
        ("focusRight", "FocusWorkspaceRight"),
        ("focusUp", "FocusWorkspaceUp"),
        ("focusDown", "FocusWorkspaceDown"),
    ]
    .map(|(id, action)| (id.to_string(), action.to_string()));
    assert_eq!(desktop_shell_actions(), expected);
    // Every hotkey that exists only to bind an app action has one.
    let shell_ids: Vec<String> = desktop_shell_actions()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    for definition in hotkey_definitions() {
        if definition.kind() == "shellAction" {
            assert!(
                shell_ids.iter().any(|id| id == definition.id),
                "{} binds no action",
                definition.id
            );
        }
    }
}

const WINDOWS_AND_LINUX: [Platform; 2] = [Platform::Windows, Platform::Linux];

#[test]
fn windows_and_linux_use_ghosttys_linux_keys() {
    for platform in WINDOWS_AND_LINUX {
        let keystrokes = default_keystrokes(platform);
        for (id, ghostty) in [
            ("pasteIntoTerminal", "ctrl-shift-v"),
            ("findInFocusedView", "ctrl-shift-f"),
            ("closeFocusedSurface", "ctrl-shift-w"),
            ("createSession", "ctrl-shift-t"),
            ("openNewWindow", "ctrl-shift-n"),
            ("splitMore", "ctrl-shift-o"),
            ("splitMoreDown", "ctrl-shift-e"),
            ("openCommandPalette", "ctrl-shift-p"),
            ("openSettings", "ctrl-,"),
            ("focusNextSession", "ctrl-tab"),
            ("focusPreviousSession", "ctrl-shift-tab"),
            ("zoomInFocusedView", "ctrl-="),
            ("zoomOutFocusedView", "ctrl--"),
            ("resetFocusedViewZoom", "ctrl-0"),
            ("focusLeft", "ctrl-alt-left"),
            ("focusRight", "ctrl-alt-right"),
            ("focusUp", "ctrl-alt-up"),
            ("focusDown", "ctrl-alt-down"),
        ] {
            assert_eq!(keystrokes[id], ghostty, "{id} on {platform:?}");
        }
    }
}

#[test]
fn windows_and_linux_ghostex_only_keys() {
    for platform in WINDOWS_AND_LINUX {
        let keystrokes = default_keystrokes(platform);
        for (id, keystroke) in [
            ("createAgentSession", "ctrl-shift-d"),
            ("openSessionSearchPalette", "ctrl-alt-shift-p"),
            ("openQuickAccessFiles", "ctrl-shift-y"),
            ("openNewThreadPalette", "ctrl-alt-shift-n"),
            ("openHotkeys", "ctrl-alt-/"),
            ("toggleSidebarCollapsed", "ctrl-shift-b"),
            ("openBrowserPane", "ctrl-shift-l"),
            ("openFindPrompts", "ctrl-alt-h"),
            ("openNotifications", "ctrl-shift-i"),
            ("exportTranscript", "ctrl-alt-x"),
            ("findNextInBrowser", "f3"),
            ("findPreviousInBrowser", "shift-f3"),
            ("sleepFocusedSession", ""),
            ("toggleAgentsFocusMode", "ctrl-alt-f"),
        ] {
            assert_eq!(keystrokes[id], keystroke, "{id} on {platform:?}");
        }
    }
}

/// CDXC:Hotkeys 2026-10-26 WHY:
/// Ctrl with a letter or `[ ] \ / @ ^ _ space` sends a control character the terminal and its programs read (Ctrl+W deletes a word, Ctrl+D ends input, Ctrl+I is Tab). The only plain Ctrl keys a default may take on Windows and Linux are Ghostty's own (Settings, zoom, next tab) and the numbered session slots.
#[test]
fn windows_and_linux_leave_plain_ctrl_keys_to_the_terminal() {
    let allowed = [
        "ctrl-,", "ctrl-=", "ctrl--", "ctrl-0", "ctrl-tab", "ctrl-1", "ctrl-2", "ctrl-3", "ctrl-4",
        "ctrl-5", "ctrl-6", "ctrl-7", "ctrl-8", "ctrl-9",
    ];
    for platform in WINDOWS_AND_LINUX {
        for (id, keystroke) in default_keystrokes(platform) {
            let Some(key) = keystroke.strip_prefix("ctrl-") else {
                continue;
            };
            assert!(
                key.contains('-') && key != "-" || allowed.contains(&keystroke.as_str()),
                "{id} takes {keystroke} from the terminal on {platform:?}"
            );
        }
    }
}

/// The chat box's Delete Line and the Files editor's Strikethrough, Highlight and Redo on Windows
/// and Linux (`native_chat/keyboard.rs`, zorite-editor): an app-wide default on one of these would
/// win over the text field.
#[test]
fn windows_and_linux_leave_text_editing_keys_alone() {
    let editing = [
        "ctrl-shift-k",
        "ctrl-shift-x",
        "ctrl-shift-h",
        "ctrl-shift-z",
    ];
    for platform in WINDOWS_AND_LINUX {
        for (id, keystroke) in default_keystrokes(platform) {
            assert!(
                !editing.contains(&keystroke.as_str()),
                "{id} takes {keystroke} on {platform:?}"
            );
        }
    }
}

#[test]
fn windows_and_linux_defaults_do_not_share_a_chord() {
    for platform in WINDOWS_AND_LINUX {
        let mut by_keystroke: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for (id, keystroke) in default_keystrokes(platform) {
            if !keystroke.is_empty() && !VIEW_SCOPED_HOTKEY_IDS.contains(&id) {
                by_keystroke.entry(keystroke).or_default().push(id);
            }
        }
        by_keystroke.retain(|_, ids| ids.len() > 1);
        assert!(by_keystroke.is_empty(), "{platform:?}: {by_keystroke:?}");
    }
}

#[test]
fn a_saved_mac_default_moves_to_the_windows_and_linux_default() {
    assert_eq!(
        platform_hotkey("splitMore", "cmd+d", Platform::Linux),
        "cmd+shift+o"
    );
    assert_eq!(
        platform_hotkey("closeFocusedSurface", "cmd+w", Platform::Windows),
        "cmd+shift+w"
    );
    assert_eq!(
        platform_hotkey("splitMore", "cmd+d", Platform::MacOs),
        "cmd+d"
    );
    // A chord the user picked stays theirs.
    assert_eq!(
        platform_hotkey("splitMore", "cmd+alt+d", Platform::Linux),
        "cmd+alt+d"
    );
}
