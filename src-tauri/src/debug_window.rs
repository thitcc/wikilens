//! The visual debug window: an always-on-top glass panel — same visual
//! language as the overlay — rendering the `debug://…` event stream (see
//! `debug.rs`) as live per-ask cards. Undecorated and transparent; the page
//! draws the glass and its drop shadow, and its header is a
//! `data-tauri-drag-region` handle (needs `core:window:allow-start-dragging`
//! in `capabilities/debug.json` — without it drag silently no-ops).
//!
//! Exists only when `WIKILENS_DEBUG` was truthy at startup — `lib.rs` decides,
//! and env can't change mid-process, so window existence always agrees with
//! the per-ask flag read. Created **hidden**, like the overlay: nothing shows
//! at launch until the user opens it (Debug chip / tray). The hidden webview
//! still loads and receives `debug://…` events, so asks run before the first
//! show are already in the history — the same mechanism that lets the hidden
//! overlay listen for hotkey events.
//! It must never steal keyboard focus from the game:
//! `focusable(false)` maps to `WS_EX_NOACTIVATE` on Windows, which covers
//! creation, clicks, drags, AND re-shows — tao's `show()` issues an
//! activating `SW_SHOW`, so `focused(false)` alone would only cover creation
//! (tao's drag loop is `WM_NCLBUTTONDOWN`/`HTCAPTION`, no activation needed).
//! Hiding: the overlay footer's Debug chip (`toggle_debug_window` command),
//! the tray's "Show debug panel", or the app-global CloseRequested handler
//! (Alt+F4 hides; the hidden webview keeps receiving events, so history
//! accumulates).

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::debug::DebugSink;

/// Window label; must match `capabilities/debug.json`.
pub const LABEL: &str = "debug";

/// Logical size of the visible glass panel; user-resizable afterwards.
const PANEL_W: f64 = 480.0;
const PANEL_H: f64 = 640.0;
const MIN_PANEL_W: f64 = 360.0;
const MIN_PANEL_H: f64 = 420.0;

/// Transparent apron around the glass so the CSS drop shadow (`--shadow-panel:
/// 0 12px 32px`) renders on every side — a draggable window has no screen
/// edge to hide a clipped shadow behind. Must match the `.debug-panel` margin
/// in src/debug/styles.css (the window.rs ↔ styles.css pairing idiom).
const APRON_TOP: f64 = 20.0;
const APRON_RIGHT: f64 = 32.0;
const APRON_BOTTOM: f64 = 44.0;
const APRON_LEFT: f64 = 32.0;

/// Create the debug window: glass at the monitor's top-left (the apron is the
/// visual gap), always-on-top, never-activating, and hidden until the Debug
/// chip or tray shows it. Called once from `.setup()` when the flag is on.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("debug.html".into()))
        .title("WikiLens Debug")
        .inner_size(PANEL_W + APRON_LEFT + APRON_RIGHT, PANEL_H + APRON_TOP + APRON_BOTTOM)
        .min_inner_size(
            MIN_PANEL_W + APRON_LEFT + APRON_RIGHT,
            MIN_PANEL_H + APRON_TOP + APRON_BOTTOM,
        )
        .resizable(true)
        // Double-clicking a drag region asks for a maximize toggle; a
        // maximized glass sheet would cover the whole game, so forbid it
        // structurally (the capability doesn't grant the command either).
        .maximizable(false)
        .decorations(false)
        .transparent(true)
        // The CSS box-shadow draws the shadow, like the overlay.
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        // Start hidden, like the overlay: the window opens only via the Debug
        // chip or tray item. The webview loads anyway, so events accumulate.
        .visible(false)
        .focused(false)
        .focusable(false)
        .build()?;
    // Created hidden like the config windows, so the first tray/chip show
    // would get DWM's one-time open transition — suppress it here too.
    crate::window::disable_os_open_transition(&win);
    if let Err(e) = position_top_left(&win) {
        eprintln!("[wikilens] failed to position debug window: {e}");
    }
    Ok(())
}

/// Re-show after the user hid the window — the tray item's action.
/// Deliberately no `set_focus`: the window is non-activating by design.
pub fn show(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.show();
    }
}

/// Hide if visible, else re-show — the overlay footer's Debug chip
/// (`toggle_debug_window`). Mirrors `window::toggle_overlay`, minus the
/// focus/emit steps: this window never takes focus and needs no re-arm event.
pub fn toggle(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(LABEL) {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            show(app);
        }
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

/// Pin to the top-left corner of the window's monitor, mirroring
/// `window::layout_overlay`'s DPI discipline (physical pixels, monitor
/// origin). No extra gap: the transparent apron already keeps the glass off
/// the corner — one geometry system, not two.
fn position_top_left(win: &WebviewWindow) -> tauri::Result<()> {
    let monitor = match win.current_monitor()? {
        Some(m) => m,
        None => match win.primary_monitor()? {
            Some(m) => m,
            None => return Ok(()), // No monitor info; leave the window where it is.
        },
    };
    let origin = monitor.position();
    win.set_position(PhysicalPosition::new(origin.x, origin.y))?;
    Ok(())
}
