//! The System, Notifications and Advanced groups of General: Auto Sleep, Power, Sounds, Sleeping
//! Sessions and Experimental.
use super::super::super::catalog::{SettingOption, settings_catalog};
use super::super::super::fields::{
    PageAction, RowSpec, action_buttons_field, reset_key, settings_section, sound_field,
    toggle_field_with,
};
use super::super::super::page::PageBlock;
use super::{GeneralCx, GeneralTab, save};
use gpui::{AnyElement, Context, Window};
use serde_json::json;
use std::rc::Rc;

fn auto_sleep_section(
    page: &mut GeneralTab,
    g: &GeneralCx,
    window: &mut Window,
    cx: &mut Context<GeneralTab>,
) -> Option<PageBlock> {
    if !g.search.subsection_visible("autoSleep", g.power_visible) {
        return None;
    }
    let s = "autoSleep";
    let options = g.options("AUTO_SLEEP_IDLE_MINUTE_OPTIONS");
    let mut rows: Vec<AnyElement> = Vec::new();
    for (key, label, description) in [
        (
            "autoSleepCodeEditorIdleMinutes",
            "VS Code Auto Sleep",
            "Choose when inactive VS Code panes sleep, or turn Auto Sleep off. Off also keeps Code awake when more than three views are open.",
        ),
        (
            "autoSleepGitEditorIdleMinutes",
            "Git Auto Sleep",
            "Choose when inactive Git panes sleep, or turn Auto Sleep off.",
        ),
        (
            "autoSleepProjectEditorIdleMinutes",
            "Project Auto Sleep",
            "Choose when inactive Project panes sleep, or turn Auto Sleep off. Off also keeps Files, Kanban and Automate awake when more than three views are open.",
        ),
        (
            "autoSleepBrowserIdleMinutes",
            "Browser Auto Sleep",
            "Choose when inactive browser panes sleep, or turn Auto Sleep off. Off also keeps the Browser awake when more than three views are open.",
        ),
        (
            "autoSleepAgentIdleMinutes",
            "Agent Auto Sleep",
            "Choose when eligible agent terminals sleep, or turn Auto Sleep off.",
        ),
    ] {
        rows.extend(page.number_select(
            g,
            s,
            key,
            label,
            description,
            options.clone(),
            false,
            window,
            cx,
        ));
        if key == "autoSleepBrowserIdleMinutes" {
            rows.extend(page.toggle(
                g,
                s,
                "keepViewsLoaded",
                "Keep views loaded",
                "Keep the side panel views you open loaded when you switch projects or workspaces, without the three-view limit or the previous project's time limit. Auto Sleep and Sleep still apply. Uses more memory.",
                false,
                cx,
            ));
        }
    }
    if g.values.f64("autoSleepAgentIdleMinutes") > 0.0 {
        rows.extend(page.toggle(
            g,
            s,
            "autoSleepRequireAgentResumeCommand",
            "Require resume command",
            "Only auto-sleep agent sessions Ghostex can wake with a resume command.",
            true,
            cx,
        ));
        rows.extend(page.toggle(
            g,
            s,
            "autoSleepFavoriteAgentSessions",
            "Include favorite agents",
            "Allow favorite agent sessions to auto-sleep.",
            true,
            cx,
        ));
    }
    settings_section(&g.p, "Auto Sleep", None, None, rows)
        .map(|section| PageBlock::section("autoSleep", section))
}

fn power_section(
    page: &mut GeneralTab,
    g: &GeneralCx,
    window: &mut Window,
    cx: &mut Context<GeneralTab>,
) -> Option<PageBlock> {
    if !g.search.subsection_visible("power", g.power_visible) {
        return None;
    }
    let s = "power";
    let mut rows: Vec<AnyElement> = Vec::new();
    rows.extend(page.toggle(
        g,
        s,
        "hideKeepAwakeTitlebarControl",
        "Hide Keep Awake",
        "Hide the Keep Awake entry from the sidebar menu.",
        false,
        cx,
    ));
    rows.extend(page.number_select(
        g,
        s,
        "keepAwakeDefaultDurationMinutes",
        "Default keep-awake duration",
        "Choose the duration Keep Awake uses by default.",
        g.options("KEEP_AWAKE_DURATION_OPTIONS"),
        false,
        window,
        cx,
    ));
    rows.extend(page.toggle(
        g,
        s,
        "keepAwakeAllowDisplaySleep",
        "Allow display sleep",
        "Keep the computer awake but allow the display to turn off.",
        false,
        cx,
    ));
    rows.extend(page.toggle(g, s, "keepAwakePreventLidSleep", "Prevent lid-close sleep", "Optional. When Keep Awake is on, Ghostex can install a small privileged helper once so closing the lid stays awake only for that active keep-awake session. Keep Awake itself remains off until you enable it.", false, cx));
    rows.extend(page.toggle(
        g,
        s,
        "keepAwakeActivateOnLaunch",
        "Activate on launch",
        "Start preventing sleep when Ghostex launches.",
        false,
        cx,
    ));
    rows.extend(page.toggle(
        g,
        s,
        "keepAwakeActivateOnExternalDisplay",
        "Activate on external display",
        "Start preventing sleep when an external display is connected.",
        false,
        cx,
    ));
    rows.extend(page.toggle(g, s, "keepAwakeWhileWorkingSessions", "Keep awake for working sessions", "Keep the computer awake while any session is Working and for 20 minutes after, so you have time to reply.", false, cx));
    let battery: Vec<SettingOption> = std::iter::once(SettingOption {
        label: "Off".to_string(),
        value: "0".to_string(),
    })
    .chain((0..17).map(|index| {
        let percent = 10 + index * 5;
        SettingOption {
            label: format!("{percent}%"),
            value: percent.to_string(),
        }
    }))
    .collect();
    rows.extend(page.number_select(
        g,
        s,
        "keepAwakeBatteryThresholdPercent",
        "Battery threshold",
        "Stop preventing sleep below this battery level, or turn the rule off.",
        battery,
        false,
        window,
        cx,
    ));
    rows.extend(page.toggle(
        g,
        s,
        "keepAwakeDeactivateOnLowPowerMode",
        "Deactivate in Low Power Mode",
        "Stop preventing sleep when Low Power Mode is enabled.",
        false,
        cx,
    ));
    rows.extend(page.toggle(
        g,
        s,
        "keepAwakeDeactivateOnUserSwitch",
        "Deactivate on user switch",
        "Stop preventing sleep when this user session is no longer active.",
        false,
        cx,
    ));
    settings_section(&g.p, "Power", None, None, rows)
        .map(|section| PageBlock::section("power", section))
}

fn post(page: &mut GeneralTab, kind: &str, cx: &mut Context<GeneralTab>) {
    let store = page.store.clone();
    super::super::super::store::post_store_message(&store, json!({ "type": kind }), cx);
}

fn sounds_section(
    page: &mut GeneralTab,
    g: &GeneralCx,
    window: &mut Window,
    cx: &mut Context<GeneralTab>,
) -> Option<PageBlock> {
    if !g.search.subsection_visible("sounds", g.power_visible) {
        return None;
    }
    let s = "sounds";
    let p = g.p;
    let mut rows: Vec<AnyElement> = Vec::new();
    // CDXC:Settings 2026-09-09 DECISION: the notification toggle leads Sounds, the two sound pickers sit together under it.
    if g.visible(s, "showMacOSAttentionNotifications") {
        let spec = g.spec(
            "showMacOSAttentionNotifications",
            "Attention Notifications",
            "Show a system notification when a session needs attention.",
        );
        rows.push(toggle_field_with(
            &p,
            "showMacOSAttentionNotifications",
            spec,
            g.values.bool("showMacOSAttentionNotifications"),
            Some(reset_key::<GeneralTab>("showMacOSAttentionNotifications")),
            |page: &mut GeneralTab, checked, _window, cx| {
                save(page, "showMacOSAttentionNotifications", json!(checked), cx);
                if checked {
                    post(page, "requestMacOSNotificationPermission", cx);
                }
            },
            cx,
        ));
    }
    if g.visible(s, "resetExpirySystemNotifications") {
        let spec = g.spec(
            "resetExpirySystemNotifications",
            "Reset Expiry Notifications",
            "Show a system notification when a banked Claude or Codex usage reset expires within 3 days and again within 24 hours. The red notification in the bell stays either way.",
        );
        rows.push(toggle_field_with(
            &p,
            "resetExpirySystemNotifications",
            spec,
            g.values.bool("resetExpirySystemNotifications"),
            Some(reset_key::<GeneralTab>("resetExpirySystemNotifications")),
            |page: &mut GeneralTab, checked, _window, cx| {
                save(page, "resetExpirySystemNotifications", json!(checked), cx);
                if checked {
                    post(page, "requestMacOSNotificationPermission", cx);
                }
            },
            cx,
        ));
    }
    if g.visible(s, "completionSound") {
        let spec = g.spec(
            "completionSound",
            "Completion Sound",
            "Sound for terminal completions.",
        );
        let value = g.values.string("completionSound");
        rows.push(sound_field(
            page,
            &p,
            "completionSound",
            spec,
            Some(reset_key::<GeneralTab>("completionSound")),
            true,
            &value,
            window,
            cx,
        ));
    }
    if g.visible(s, "actionCompletionSound") {
        let spec = g.spec(
            "actionCompletionSound",
            "Action Completion Sound",
            "Sound for action completions.",
        );
        let value = g.values.string("actionCompletionSound");
        rows.push(sound_field(
            page,
            &p,
            "actionCompletionSound",
            spec,
            Some(reset_key::<GeneralTab>("actionCompletionSound")),
            false,
            &value,
            window,
            cx,
        ));
    }
    rows.extend(page.toggle(
        g,
        s,
        "copySound",
        "Copy Sound",
        "Play a short sound when copying to the clipboard, including text from the chat composer.",
        false,
        cx,
    ));
    // CDXC:Notifications 2026-05-11-01:14: the test runs the real completion alert path.
    if g.visible(s, "attentionNotificationActions") {
        let spec = RowSpec::new("Completion Alerts")
            .description("Run the current completion sound and notification flow, or open system notification permissions.")
            .advanced(settings_catalog().is_advanced("attentionNotificationActions"));
        let test: PageAction<GeneralTab> =
            Rc::new(|page: &mut GeneralTab, _window, cx| post(page, "testAgentTaskCompletion", cx));
        let settings: PageAction<GeneralTab> = Rc::new(|page: &mut GeneralTab, _window, cx| {
            post(page, "openMacOSNotificationSettings", cx)
        });
        rows.push(action_buttons_field(
            &p,
            "attentionNotificationActions",
            spec,
            vec![
                ("Test agent task completion", test),
                ("Notification Settings", settings),
            ],
            cx,
        ));
    }
    settings_section(&p, "Sounds", None, None, rows)
        .map(|section| PageBlock::section("notifications", section))
}

/// The two sleeping-session toggles (CDXC:SessionSleep 2026-09-29 DECISION in
/// packages/shared/ghostex-settings/types.ts (deleted 2026-10-01)), on the Sidebar & Sessions page.
pub(super) fn sleeping_sessions_section(
    page: &mut GeneralTab,
    g: &GeneralCx,
    cx: &mut Context<GeneralTab>,
) -> Option<PageBlock> {
    if !g
        .search
        .subsection_visible("sleepingSessions", g.power_visible)
    {
        return None;
    }
    let mut rows: Vec<AnyElement> = Vec::new();
    rows.extend(page.toggle(
        g,
        "sleepingSessions",
        "dimSleepingSessions",
        "Dim sleeping sessions",
        "Fade sleeping sessions in the sidebar so they stand apart from awake ones.",
        false,
        cx,
    ));
    rows.extend(page.toggle(
        g,
        "sleepingSessions",
        "wakeSleepingSessionsOnSelect",
        "Wake sleeping sessions when selected",
        "Wake a sleeping session as soon as you select it. Turn off to open it with a Resume button instead, so switching sessions does not wake it by accident.",
        false,
        cx,
    ));
    settings_section(&g.p, "Sleeping Sessions", None, None, rows)
        .map(|section| PageBlock::section("sleepingSessions", section))
}

/// The Power & Sleep page (the old System group: CDXC:Settings 2026-10-29 in model.rs).
pub(super) fn power_sleep_sections(
    page: &mut GeneralTab,
    g: &GeneralCx,
    window: &mut Window,
    cx: &mut Context<GeneralTab>,
) -> Vec<PageBlock> {
    [
        auto_sleep_section(page, g, window, cx),
        power_section(page, g, window, cx),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The Notifications page.
pub(super) fn notifications_sections(
    page: &mut GeneralTab,
    g: &GeneralCx,
    window: &mut Window,
    cx: &mut Context<GeneralTab>,
) -> Vec<PageBlock> {
    sounds_section(page, g, window, cx).into_iter().collect()
}
