//! The Windows-only half of foreground detection: read the process and
//! caption of the window the overlay is about to cover.
//!
//! Best-effort by design — every failure leg returns `None`. A summon must
//! never surface an error because the player happened to be over the desktop,
//! over a protected process, or over a window that vanished mid-call.
//!
//! Anti-cheat posture (vault/2026-08-04_game-auto-detection.md): the handle is
//! opened with `PROCESS_QUERY_LIMITED_INFORMATION` and nothing else — never
//! `PROCESS_QUERY_INFORMATION`, never `PROCESS_VM_READ`, the rights kernel
//! anti-cheats actually score — and it is closed within microseconds and never
//! cached. Reading a process *name* is what Task Manager and every game-capture
//! tool does; this deliberately stays on that side of the line.

/// The foreground window's full image path and raw window caption.
///
/// `None` when there is no foreground window, when it belongs to WikiLens
/// itself, or when the process could not be read. The caller reduces the path
/// with [`super::reduce`] — the full path never leaves Rust.
#[cfg(windows)]
pub fn probe() -> Option<(String, String)> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_INSUFFICIENT_BUFFER};
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    // Documented to return NULL "when a window is losing activation".
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }

    let mut pid: u32 = 0;
    let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if thread == 0 || pid == 0 {
        return None;
    }
    // The one guard that covers every window we own — the overlay, the capture
    // and debug windows, and the hidden message windows the global-shortcut
    // plugin and the tray keep alive. Comparing PIDs rather than HWNDs is what
    // makes it total, and what makes a future second caller inherit it.
    if pid == unsafe { GetCurrentProcessId() } {
        return None;
    }

    // One fixed-size read. `GetWindowTextLengthW` is deliberately unused —
    // Microsoft documents it may over-report. Cross-process, `GetWindowText`
    // reads the caption directly instead of sending `WM_GETTEXT`, so a hung
    // game cannot stall the summon path.
    let mut title_buf = [0u16; 512];
    let copied = unsafe { GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32) };
    let title = if copied > 0 {
        String::from_utf16_lossy(&title_buf[..copied as usize])
    } else {
        String::new()
    };

    // `OpenProcess` signals failure with NULL (not INVALID_HANDLE_VALUE).
    // Denied means a protected or higher-integrity process, or one that has
    // already exited — both are "unknown", never an error.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }

    // Straight-line from here to CloseHandle: no early return may skip it.
    // `lpdwSize` is in/out in characters — capacity in, count written (minus
    // the NUL) out. Only a too-small buffer is worth a retry; every other
    // failure is final.
    let mut path: Option<String> = None;
    for capacity in [1024usize, 32768] {
        let mut buf = vec![0u16; capacity];
        let mut len = capacity as u32;
        let ok = unsafe {
            QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len)
        };
        if ok != 0 {
            buf.truncate(len as usize);
            path = Some(String::from_utf16_lossy(&buf));
            break;
        }
        if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
            break;
        }
    }
    let _ = unsafe { CloseHandle(handle) };

    path.map(|p| (p, title))
}

/// Non-Windows stub. WikiLens ships Windows-only; this exists so the crate
/// still builds on a developer's other machine, mirroring
/// `window::disable_os_open_transition`. An honest `None` — never a fake
/// success, which would let a bogus detection reach the matcher.
#[cfg(not(windows))]
pub fn probe() -> Option<(String, String)> {
    None
}
