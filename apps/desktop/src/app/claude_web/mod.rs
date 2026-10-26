//! Claude, a built-in extension: claude.ai in a view opened from the sidebar's bottom bar, on the
//! picked saved Claude account's own claude.ai sign-in. Per-concern files; this barrel stays thin.
mod accounts;
mod sidebar_button;
mod view;

pub(crate) use accounts::*;
pub(crate) use view::*;
