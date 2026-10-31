//! `target::sessions` (T38, plan D47): the terminal windows a Translate tab can be bound to,
//! binding by focus, and the liveness watcher that reports closed bound windows.
//!
//! - **Listing:** visible top-level Windows Terminal windows (one target per tab, read with UI
//!   Automation) and classic console windows, with their titles. The two hosts T39 can paste
//!   into (`docs/spikes/session-paste.md`); the other terminals of POPUP-007 aren't targets.
//! - **Translator runs are never targets** (D41): a window is skipped when its process is one of
//!   the app's translator runs or an ancestor of one (e.g. the terminal the app was started from
//!   in dev mode). Translator runs have no window of their own (`CREATE_NO_WINDOW`).
//! - **Bindings** are counted per target (two Translate tabs may share one). While any exists, a
//!   watcher thread checks every [`WATCH_INTERVAL`] that each window still exists with the same
//!   pid and class, and that a bound tab is still there; it emits [`CLOSED_EVENT`] for the ones
//!   that are gone and forgets them. Without bindings the watcher thread isn't running.
//!
//! Nothing here focuses a window, selects a tab, types or touches the clipboard.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use super::{TargetClosed, TerminalTarget};
use crate::error::{AppError, AppResult};
use crate::translation::providers::claude_code::TranslatorPids;

/// Emitted (to every window) when a bound terminal window or tab has closed.
pub const CLOSED_EVENT: &str = "target://closed";

/// At most this many bindings at a time (plan D47: one default tab plus up to 5 bound ones).
pub const MAX_BINDINGS: usize = 5;

/// How often the watcher checks the bound windows.
pub const WATCH_INTERVAL: Duration = Duration::from_secs(2);

/// `bind_terminal_by_focus` waits at most this long, whatever the caller asks.
pub const MAX_FOCUS_TIMEOUT: Duration = Duration::from_secs(60);

/// How often `bind_terminal_by_focus` looks at the foreground window.
const FOCUS_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// What identifies a binding: the window and, for Windows Terminal, the tab.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BindingKey {
    hwnd: u64,
    tab_runtime_id: Option<String>,
}

impl BindingKey {
    fn of(target: &TerminalTarget) -> Self {
        Self {
            hwnd: target.hwnd,
            tab_runtime_id: target.tab_runtime_id.clone(),
        }
    }
}

struct Bound {
    target: TerminalTarget,
    /// How many Translate tabs are bound to this target.
    count: usize,
}

#[derive(Default)]
struct Inner {
    bound: HashMap<BindingKey, Bound>,
    watcher_running: bool,
}

/// The bound terminal targets (managed state). Clones are handles to the same set.
#[derive(Clone, Default)]
pub struct Bindings(Arc<Mutex<Inner>>);

impl Bindings {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // The map stays consistent even if a holder panicked.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `InvalidInput` when [`MAX_BINDINGS`] bindings exist already.
    pub fn ensure_room(&self) -> AppResult<()> {
        Self::room_in(&self.lock())
    }

    fn room_in(inner: &Inner) -> AppResult<()> {
        let total: usize = inner.bound.values().map(|b| b.count).sum();
        if total >= MAX_BINDINGS {
            return Err(AppError::InvalidInput(format!(
                "At most {MAX_BINDINGS} Translate tabs can be bound to Claude sessions"
            )));
        }
        Ok(())
    }

    /// Adds one binding to `target` and starts the watcher if it isn't running. `InvalidInput`
    /// when [`MAX_BINDINGS`] bindings exist already.
    pub fn bind(&self, app: &AppHandle, target: TerminalTarget) -> AppResult<()> {
        let mut inner = self.lock();
        Self::room_in(&inner)?;
        inner
            .bound
            .entry(BindingKey::of(&target))
            .and_modify(|b| {
                b.count += 1;
                b.target.title.clone_from(&target.title);
            })
            .or_insert(Bound { target, count: 1 });
        if !inner.watcher_running {
            inner.watcher_running = true;
            let app = app.clone();
            let bindings = self.clone();
            let spawned = std::thread::Builder::new()
                .name("session-watcher".into())
                .spawn(move || watch(&app, &bindings));
            if let Err(e) = spawned {
                inner.watcher_running = false;
                log::warn!("Couldn't start the session watcher: {e}");
            }
        }
        Ok(())
    }

    /// Removes one binding to `target` (nothing when it isn't bound, e.g. after it closed).
    pub fn unbind(&self, target: &TerminalTarget) {
        let mut inner = self.lock();
        let key = BindingKey::of(target);
        if let Some(bound) = inner.bound.get_mut(&key) {
            bound.count -= 1;
            if bound.count == 0 {
                inner.bound.remove(&key);
            }
        }
    }

    /// The bound targets, or `None` (and the watcher marked stopped) when there are none.
    fn targets_or_stop(&self) -> Option<Vec<TerminalTarget>> {
        let mut inner = self.lock();
        if inner.bound.is_empty() {
            inner.watcher_running = false;
            return None;
        }
        Some(inner.bound.values().map(|b| b.target.clone()).collect())
    }

    /// Forgets `target`'s bindings; `true` if it was bound.
    fn forget(&self, target: &TerminalTarget) -> bool {
        self.lock().bound.remove(&BindingKey::of(target)).is_some()
    }
}

/// The watcher thread: every [`WATCH_INTERVAL`], checks the bound targets and reports the closed
/// ones; returns as soon as nothing is bound.
fn watch(app: &AppHandle, bindings: &Bindings) {
    log::info!("Session watcher started");
    let checker = platform::LivenessChecker::new();
    loop {
        std::thread::sleep(WATCH_INTERVAL);
        let Some(targets) = bindings.targets_or_stop() else {
            log::info!("Session watcher stopped: no bindings");
            return;
        };
        for target in checker.closed(&targets) {
            if bindings.forget(&target) {
                log::info!(
                    "Bound terminal closed (window {:#x}, tab {})",
                    target.hwnd,
                    target.tab_runtime_id.as_deref().unwrap_or("-")
                );
                let payload = TargetClosed {
                    hwnd: target.hwnd,
                    tab_runtime_id: target.tab_runtime_id,
                };
                if let Err(e) = app.emit(CLOSED_EVENT, payload) {
                    log::warn!("Couldn't emit {CLOSED_EVENT}: {e}");
                }
            }
        }
    }
}

/// The pids whose windows are never targets: the live translator runs and their ancestors.
fn excluded_pids(translators: &TranslatorPids) -> std::collections::HashSet<u32> {
    let pids = translators.pids();
    if pids.is_empty() {
        return Default::default();
    }
    crate::system::process::with_ancestors(pids, &crate::system::process::parent_map())
}

/// Every terminal target that can be bound now (blocking: UI Automation; call it off the UI
/// thread).
pub fn list(translators: &TranslatorPids) -> Vec<TerminalTarget> {
    let targets = platform::list(&excluded_pids(translators));
    log::info!("Listed {} terminal session target(s)", targets.len());
    targets
}

/// Waits up to `timeout` (capped at [`MAX_FOCUS_TIMEOUT`]) for the foreground to move to a
/// terminal window, then returns it with its selected tab. `Timeout` when none came to the
/// front. Blocking; call it off the UI thread.
pub fn wait_for_focused_terminal(
    translators: &TranslatorPids,
    timeout: Duration,
) -> AppResult<TerminalTarget> {
    platform::wait_for_focused_terminal(translators, timeout.min(MAX_FOCUS_TIMEOUT))
}

/// The target as it is now (fresh title), or an error when it can't be bound: the window or tab
/// is gone (`SessionClosed`), or it belongs to a translator run (`InvalidInput`). Blocking.
pub fn check(translators: &TranslatorPids, target: &TerminalTarget) -> AppResult<TerminalTarget> {
    platform::check(&excluded_pids(translators), target)
}

#[cfg(windows)]
mod platform {
    use std::collections::{HashMap, HashSet};
    use std::time::{Duration, Instant};

    use super::{FOCUS_POLL_INTERVAL, excluded_pids};
    use crate::error::{AppError, AppResult};
    use crate::system::input::{self, CONHOST_CLASS, WINDOWS_TERMINAL_CLASS, Window};
    use crate::system::uia::{TabInfo, Uia};
    use crate::target::{TerminalHost, TerminalTarget};
    use crate::translation::providers::claude_code::TranslatorPids;

    fn host_of(class: &str) -> Option<TerminalHost> {
        if class.eq_ignore_ascii_case(WINDOWS_TERMINAL_CLASS) {
            Some(TerminalHost::WindowsTerminal)
        } else if class.eq_ignore_ascii_case(CONHOST_CLASS) {
            Some(TerminalHost::Conhost)
        } else {
            None
        }
    }

    fn new_uia() -> Option<Uia> {
        match Uia::new() {
            Ok(uia) => Some(uia),
            Err(e) => {
                log::warn!("UI Automation unavailable, Windows Terminal tabs aren't read: {e}");
                None
            }
        }
    }

    fn read_tabs(uia: Option<&Uia>, window: Window) -> Option<Vec<TabInfo>> {
        match uia?.tabs(window.id()) {
            Ok(tabs) => Some(tabs),
            Err(e) => {
                log::warn!("Couldn't read the tabs of window {:#x}: {e}", window.id());
                None
            }
        }
    }

    /// A visible window of a supported terminal host that isn't a translator run's: its host
    /// and pid.
    fn terminal_window(window: Window, excluded: &HashSet<u32>) -> Option<(TerminalHost, u32)> {
        if !window.is_visible() {
            return None;
        }
        let host = host_of(&window.class_name())?;
        let pid = window.pid();
        (pid != 0 && !excluded.contains(&pid)).then_some((host, pid))
    }

    fn window_target(
        window: Window,
        host: TerminalHost,
        pid: u32,
        tab: Option<&TabInfo>,
    ) -> TerminalTarget {
        let title = tab
            .map(|t| t.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| window.title());
        TerminalTarget {
            hwnd: window.id() as u64,
            pid,
            tab_runtime_id: tab.map(|t| t.runtime_id.clone()),
            title,
            host,
        }
    }

    pub fn list(excluded: &HashSet<u32>) -> Vec<TerminalTarget> {
        let mut uia: Option<Option<Uia>> = None;
        let mut targets = Vec::new();
        for window in input::top_level_windows() {
            let Some((host, pid)) = terminal_window(window, excluded) else {
                continue;
            };
            if host == TerminalHost::WindowsTerminal {
                let uia = uia.get_or_insert_with(new_uia).as_ref();
                if let Some(tabs) = read_tabs(uia, window).filter(|t| !t.is_empty()) {
                    targets.extend(
                        tabs.iter()
                            .map(|tab| window_target(window, host, pid, Some(tab))),
                    );
                    continue;
                }
            }
            targets.push(window_target(window, host, pid, None));
        }
        targets
    }

    pub fn wait_for_focused_terminal(
        translators: &TranslatorPids,
        timeout: Duration,
    ) -> AppResult<TerminalTarget> {
        let deadline = Instant::now() + timeout;
        let start = input::foreground_window();
        let mut moved = false;
        let mut uia: Option<Option<Uia>> = None;
        loop {
            if let Some(window) = input::foreground_window() {
                // Only a window brought to the front after the call counts, so a terminal that
                // was already in front must be left and re-entered.
                moved |= Some(window) != start;
                if moved
                    && let Some((host, pid)) = terminal_window(window, &excluded_pids(translators))
                {
                    let tab = if host == TerminalHost::WindowsTerminal {
                        let uia = uia.get_or_insert_with(new_uia).as_ref();
                        read_tabs(uia, window)
                            .and_then(|tabs| tabs.into_iter().find(|t| t.selected))
                    } else {
                        None
                    };
                    let target = window_target(window, host, pid, tab.as_ref());
                    log::info!(
                        "Terminal brought to the front for binding (window {:#x})",
                        target.hwnd
                    );
                    return Ok(target);
                }
            }
            if Instant::now() >= deadline {
                return Err(AppError::Timeout);
            }
            std::thread::sleep(FOCUS_POLL_INTERVAL);
        }
    }

    pub fn check(excluded: &HashSet<u32>, target: &TerminalTarget) -> AppResult<TerminalTarget> {
        let window = Window::from_id(target.hwnd as usize);
        if !window.exists()
            || window.pid() != target.pid
            || host_of(&window.class_name()) != Some(target.host)
        {
            return Err(AppError::SessionClosed);
        }
        if excluded.contains(&target.pid) {
            return Err(AppError::InvalidInput(
                "This window runs one of the app's own translations and can't be bound".into(),
            ));
        }
        let Some(runtime_id) = &target.tab_runtime_id else {
            return Ok(window_target(window, target.host, target.pid, None));
        };
        let uia = new_uia();
        let tabs = read_tabs(uia.as_ref(), window).ok_or_else(|| {
            AppError::Io("Can't read the tabs of this Windows Terminal window".into())
        })?;
        let tab = tabs
            .iter()
            .find(|t| &t.runtime_id == runtime_id)
            .ok_or(AppError::SessionClosed)?;
        Ok(window_target(window, target.host, target.pid, Some(tab)))
    }

    /// The watcher's checks. Owns the UI Automation client, so it lives on the watcher thread.
    pub struct LivenessChecker {
        uia: Option<Uia>,
    }

    impl LivenessChecker {
        pub fn new() -> Self {
            Self { uia: new_uia() }
        }

        /// The targets whose window is gone (or now belongs to another process or class) or
        /// whose tab is gone. When a window's tabs can't be read, its tabs count as alive: a
        /// passing UI Automation error must not close the user's Translate tab.
        pub fn closed(&self, targets: &[TerminalTarget]) -> Vec<TerminalTarget> {
            let mut tabs_by_window: HashMap<u64, Option<Vec<TabInfo>>> = HashMap::new();
            let mut closed = Vec::new();
            for target in targets {
                let window = Window::from_id(target.hwnd as usize);
                let window_alive = window.exists()
                    && window.pid() == target.pid
                    && host_of(&window.class_name()) == Some(target.host);
                let alive = window_alive
                    && target.tab_runtime_id.as_ref().is_none_or(|runtime_id| {
                        tabs_by_window
                            .entry(target.hwnd)
                            .or_insert_with(|| read_tabs(self.uia.as_ref(), window))
                            .as_ref()
                            .is_none_or(|tabs| tabs.iter().any(|t| &t.runtime_id == runtime_id))
                    });
                if !alive {
                    closed.push(target.clone());
                }
            }
            closed
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::collections::HashSet;
    use std::time::Duration;

    use crate::error::{AppError, AppResult};
    use crate::target::TerminalTarget;
    use crate::translation::providers::claude_code::TranslatorPids;

    pub fn list(_excluded: &HashSet<u32>) -> Vec<TerminalTarget> {
        Vec::new()
    }

    pub fn wait_for_focused_terminal(
        _translators: &TranslatorPids,
        _timeout: Duration,
    ) -> AppResult<TerminalTarget> {
        Err(AppError::Timeout)
    }

    pub fn check(_excluded: &HashSet<u32>, _target: &TerminalTarget) -> AppResult<TerminalTarget> {
        Err(AppError::SessionClosed)
    }

    pub struct LivenessChecker;

    impl LivenessChecker {
        pub fn new() -> Self {
            Self
        }

        pub fn closed(&self, targets: &[TerminalTarget]) -> Vec<TerminalTarget> {
            targets.to_vec()
        }
    }
}
