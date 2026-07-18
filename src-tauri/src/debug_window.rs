//! The visual debug window: an always-on-top, opaque tool window rendering
//! the `debug://…` event stream (see `debug.rs`) as live per-ask cards.
//!
//! Exists only when `WIKILENS_DEBUG` was truthy at startup — `lib.rs` decides,
//! and env can't change mid-process, so window existence always agrees with
//! the per-ask flag read. It must never steal keyboard focus from the game:
//! `focusable(false)` maps to `WS_EX_NOACTIVATE` on Windows, which covers
//! creation, clicks, AND re-shows — tao's `show()` issues an activating
//! `SW_SHOW`, so `focused(false)` alone would only cover creation. Closing is
//! the app-global CloseRequested handler (hides; the hidden webview keeps
//! receiving events, so history accumulates); the tray's "Show debug panel"
//! item re-shows it.

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::debug::DebugSink;

/// Window label; must match `capabilities/debug.json`.
pub const LABEL: &str = "debug";

/// Logical inner size at creation; user-resizable afterwards.
const WIDTH: f64 = 480.0;
const HEIGHT: f64 = 640.0;
const MIN_WIDTH: f64 = 360.0;
const MIN_HEIGHT: f64 = 420.0;

/// Logical gap from the monitor's top-left corner. The overlay docks
/// top-right, so the two can never overlap.
const GAP: u32 = 12;

/// Create the debug window: top-left, opaque, decorated, always-on-top,
/// never-activating. Called once from `.setup()` when the flag is on.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("debug.html".into()))
        .title("WikiLens Debug")
        .inner_size(WIDTH, HEIGHT)
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .resizable(true)
        .decorations(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .build()?;
    if let Err(e) = position_top_left(&win) {
        eprintln!("[wikilens] failed to position debug window: {e}");
    }
    Ok(())
}

/// Re-show after the user closed (= hid) the window — the tray item's action.
/// Deliberately no `set_focus`: the window is non-activating by design.
pub fn show(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.show();
    }
}

/// The `debug://…` outlet for `DebugReport::attach_sink`: `Some` only when
/// the window exists. `emit_to` is synchronous (Drop-safe) and targets the
/// debug webview only — the overlay never sees these events.
pub fn sink(app: &AppHandle) -> Option<DebugSink> {
    app.get_webview_window(LABEL)?;
    let handle = app.clone();
    Some(Box::new(move |event, payload| {
        let _ = handle.emit_to(LABEL, event, payload);
    }))
}

/// Pin to the top-left of the window's monitor, mirroring
/// `window::position_top_right`'s DPI discipline: physical pixels throughout,
/// monitor origin plus the scaled logical gap.
fn position_top_left(win: &WebviewWindow) -> tauri::Result<()> {
    let monitor = match win.current_monitor()? {
        Some(m) => m,
        None => match win.primary_monitor()? {
            Some(m) => m,
            None => return Ok(()), // No monitor info; leave the window where it is.
        },
    };
    let origin = monitor.position();
    let gap = (GAP as f64 * monitor.scale_factor()).round() as i32;
    win.set_position(PhysicalPosition::new(origin.x + gap, origin.y + gap))?;
    Ok(())
}
