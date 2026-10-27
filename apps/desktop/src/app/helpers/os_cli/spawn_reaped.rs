//! Programs the app starts and never talks to again: a sound, an opener, an editor launcher.

/// CDXC:OsIntegration 2026-10-26 WHY:
/// On Unix a dropped `Child` is never reaped, so each program started that way stayed a zombie until Ghostex quit (the completion sound's `afplay` left one per sound). Each child gets a waiting thread of its own, as the terminal children do (`terminal_model/child_lifecycle.rs`), so a long-running one such as the editor daemon cannot hold up the others the way it would behind one serial waiter; changing the process-wide SIGCHLD policy would break the other owners' waits.
pub(crate) trait SpawnReaped {
    /// Starts the program and waits for it in the background.
    fn spawn_reaped(&mut self) -> std::io::Result<()>;
}

impl SpawnReaped for std::process::Command {
    fn spawn_reaped(&mut self) -> std::io::Result<()> {
        let mut child = self.spawn()?;
        // A thread that cannot start leaves the program unreaped, as before.
        let _ = std::thread::Builder::new()
            .name("ghostex-child-wait".into())
            .spawn(move || {
                let _ = child.wait();
            });
        Ok(())
    }
}
