//! The Settings > Hotkeys rows that run an app action directly (paste, find, zoom, close, new
//! window, Focus mode, and the shell's own copy of the sidebar, view panel, new tab, split and
//! pane-focus keys), bound at the chord Settings gives them and bound again after every save.
//!
//! CDXC:Hotkeys 2026-10-26 WHY:
//! These keys were literal bindings in main.rs, so Settings could neither move nor clear them and, on Windows and Linux, Ctrl+W, Ctrl+D, Ctrl+F, Ctrl+T and Ctrl+V never reached the terminal (issue #315). They stay bound to their own actions rather than `RunConfiguredGhostexHotkey`, at the same chords, so macOS behaves exactly as before; only the chord now comes from the hotkey catalog and the saved settings (CDXC:Hotkeys 2026-10-26 DECISION in packages/settings-catalog/src/hotkey_definitions.rs).
//! SEE-ALSO: packages/settings-catalog/tests/platform_hotkeys.rs reads `SHELL_HOTKEY_ACTIONS` from this file to pin which action each row runs.

use crate::*;

/// The catalog id each app action follows, in the order main.rs bound them. The `shellAction`
/// rows exist only for this; the others are catalog hotkeys whose `RunConfiguredGhostexHotkey`
/// binding wins at the same chord, with the action here as its fallback.
const SHELL_HOTKEY_ACTIONS: &[(&str, fn() -> Box<dyn Action>)] = &[
    ("pasteIntoTerminal", || Box::new(PasteIntoFocusedTerminal)),
    ("findInFocusedView", || Box::new(FindInFocusedTerminal)),
    ("findNextInBrowser", || Box::new(FindNextInFocusedBrowser)),
    ("findPreviousInBrowser", || {
        Box::new(FindPreviousInFocusedBrowser)
    }),
    ("zoomInFocusedView", || Box::new(ZoomInFocusedSurface)),
    ("zoomOutFocusedView", || Box::new(ZoomOutFocusedSurface)),
    ("resetFocusedViewZoom", || Box::new(ResetFocusedSurfaceZoom)),
    ("closeFocusedSurface", || Box::new(CloseFocusedSurface)),
    ("toggleSidebarCollapsed", || {
        Box::new(ToggleGpuiSidebarCollapsed)
    }),
    ("toggleViewPanel", || Box::new(ToggleViewPanel)),
    ("createSession", || Box::new(NewTerminalTab)),
    ("openNewWindow", || Box::new(NewGhostexGpuiWindow)),
    ("splitMore", || Box::new(SplitFocusedTerminalRight)),
    ("splitMoreDown", || Box::new(SplitFocusedTerminalDown)),
    ("openBrowserPane", || Box::new(NewBrowserTab)),
    ("toggleAgentsFocusMode", || Box::new(ToggleAgentsFocusMode)),
    ("mergeAllTabs", || Box::new(MergeAllTabs)),
    ("focusLeft", || Box::new(FocusWorkspaceLeft)),
    ("focusRight", || Box::new(FocusWorkspaceRight)),
    ("focusUp", || Box::new(FocusWorkspaceUp)),
    ("focusDown", || Box::new(FocusWorkspaceDown)),
];

/// Surfaces that are not terminals, where Ctrl+F also runs Find on Windows and Linux (the chat,
/// browser pages and other web views). The Files view answers Ctrl+F itself (`native_docs`).
const NON_TERMINAL_FIND_CONTEXTS: &[&str] = &["NativeChat", BROWSER_KEY_CONTEXT, CEF_KEY_CONTEXT];

fn key_binding(
    keystroke: &str,
    action: Box<dyn Action>,
    context: Option<&str>,
    cx: &gpui::App,
) -> Option<KeyBinding> {
    if !gpui_runtime_keystroke_parses(keystroke, cx) {
        return None;
    }
    let context = context.map(|context| {
        gpui::KeyBindingContextPredicate::parse(context)
            .unwrap_or_else(|error| panic!("invalid key context {context}: {error}"))
            .into()
    });
    KeyBinding::load(keystroke, action, context, false, None, cx).ok()
}

/// Every binding of these actions, each as `(keystroke, action, context)`, read from `snapshot`.
fn shell_hotkey_bindings(
    snapshot: &shared_settings::SharedSidebarSettingsSnapshot,
) -> Vec<(String, fn() -> Box<dyn Action>, Option<&'static str>)> {
    let keystroke = |action_id: &str| {
        gpui_configured_hotkey_key_in(snapshot, action_id)
            .and_then(|key| gpui_keystroke_from_shared_hotkey(&key))
    };
    let mut bindings = Vec::new();
    for (action_id, action) in SHELL_HOTKEY_ACTIONS {
        // Paste is a terminal key on Windows and Linux (CDXC:Clipboard 2026-10-26 DECISION in
        // packages/settings-catalog/src/hotkeys.rs).
        let context = ghostex_settings_catalog::hotkey_is_terminal_only(
            action_id,
            ghostex_settings_catalog::Platform::current(),
        )
        .then_some(terminal_element::TERMINAL_KEY_CONTEXT);
        if let Some(keystroke) = keystroke(action_id) {
            bindings.push((keystroke, *action, context));
        }
        // Ghostty's `shift+insert=paste_from_clipboard`.
        if *action_id == "pasteIntoTerminal" {
            bindings.push((
                "shift-insert".to_string(),
                *action,
                Some(terminal_element::TERMINAL_KEY_CONTEXT),
            ));
        }
        // Zoom In's second key, the `+` the same key types with Shift, while Zoom In is bound.
        if *action_id == "zoomInFocusedView" && keystroke(action_id).is_some() {
            let plus = if cfg!(target_os = "macos") {
                "cmd-+"
            } else {
                "ctrl-+"
            };
            bindings.push((plus.to_string(), *action, None));
        }
        // CDXC:Hotkeys 2026-10-26 WHY:
        // Find is Ctrl+Shift+F on Windows and Linux, Ghostty's key, so Ctrl+F reaches the terminal; the chat, browser pages and the Files view (which answers it itself) are not terminals and keep Ctrl+F too, as their help text and the Files DECISION of 2026-09-12 promise.
        if *action_id == "findInFocusedView"
            && !cfg!(target_os = "macos")
            && keystroke(action_id).is_some_and(|keystroke| keystroke != "ctrl-f")
        {
            for context in NON_TERMINAL_FIND_CONTEXTS {
                bindings.push(("ctrl-f".to_string(), *action, Some(*context)));
            }
        }
    }
    bindings
}

/// The bindings for the current settings. Bind them before
/// `gpui_configured_hotkey_key_bindings_from_settings`, so a configured hotkey on the same chord
/// keeps precedence.
pub(crate) fn gpui_shell_hotkey_key_bindings_from_settings(cx: &gpui::App) -> Vec<KeyBinding> {
    shell_hotkey_bindings(&shared_settings::shared_sidebar_settings_snapshot())
        .into_iter()
        .filter_map(|(keystroke, action, context)| key_binding(&keystroke, action(), context, cx))
        .collect()
}

/// Unbind markers for the bindings `snapshot` made, added before a save rebinds them.
pub(crate) fn gpui_shell_hotkey_unbinds_from_settings(
    snapshot: &shared_settings::SharedSidebarSettingsSnapshot,
    cx: &gpui::App,
) -> Vec<KeyBinding> {
    shell_hotkey_bindings(snapshot)
        .into_iter()
        .filter_map(|(keystroke, action, _)| {
            key_binding(
                &keystroke,
                Box::new(gpui::Unbind(action().name().into())),
                None,
                cx,
            )
        })
        .collect()
}

/// The shell bindings and then the configured hotkeys for the current settings, in that order.
pub(crate) fn gpui_bind_hotkeys_from_settings(cx: &mut gpui::App) {
    let shell_hotkeys = gpui_shell_hotkey_key_bindings_from_settings(cx);
    cx.bind_keys(shell_hotkeys);
    let configured_hotkeys = gpui_configured_hotkey_key_bindings_from_settings(cx);
    cx.bind_keys(configured_hotkeys);
}
