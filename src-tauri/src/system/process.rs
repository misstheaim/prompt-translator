//! `system::process`: the process tree (parent pids from a ToolHelp snapshot), used to keep the
//! app's own translator runs out of the session targets (plan D41).

use std::collections::{HashMap, HashSet};

/// Every live process's parent: pid → parent pid.
#[cfg(windows)]
pub fn parent_map() -> HashMap<u32, u32> {
    use std::mem::size_of;

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let mut parents = HashMap::new();
    // SAFETY: a process snapshot of the whole system; the pid argument is ignored.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        log::warn!(
            "Process snapshot failed: {}",
            std::io::Error::last_os_error()
        );
        return parents;
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: `entry` is a PROCESSENTRY32W with `dwSize` set, as the ToolHelp functions require.
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while more {
        parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    // SAFETY: the snapshot handle was created above and is closed exactly once.
    unsafe { CloseHandle(snapshot) };
    parents
}

#[cfg(not(windows))]
pub fn parent_map() -> HashMap<u32, u32> {
    HashMap::new()
}

/// `pids` plus all their ancestors in `parents`. A parent pid may name a process that has
/// exited (or a reused id); that only adds pids, which callers treat as "skip", the safe side.
pub fn with_ancestors(
    pids: impl IntoIterator<Item = u32>,
    parents: &HashMap<u32, u32>,
) -> HashSet<u32> {
    /// Deeper chains than this don't occur; the cap also stops pid cycles from reuse.
    const MAX_DEPTH: usize = 64;
    let mut found = HashSet::new();
    for pid in pids {
        let mut current = pid;
        for _ in 0..MAX_DEPTH {
            if current == 0 || !found.insert(current) {
                break;
            }
            match parents.get(&current) {
                Some(&parent) if parent != current => current = parent,
                _ => break,
            }
        }
    }
    found
}
