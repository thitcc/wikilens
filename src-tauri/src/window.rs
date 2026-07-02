//! Overlay window control: toggle/show/hide and right-edge docking.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Window label from `tauri.conf.json`.
pub const OVERLAY_LABEL: &str = "overlay";
/// Logical panel width in CSS pixels. Physical width is scaled per-monitor.
pub const PANEL_WIDTH: u32 = 420;

/// Event emitted after the panel is shown so the frontend can focus the input.
const EVENT_SHOWN: &str = "overlay://shown";

fn overlay_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(OVERLAY_LABEL)
}

/// Toggle the overlay: hide if visible, otherwise dock it to the right edge,
/// show it, take focus, and notify the frontend.
pub fn toggle_overlay(app: &AppHandle) {
    let Some(win) = overlay_window(app) else {
        eprintln!("[wikilens] overlay window '{OVERLAY_LABEL}' not found");
        return;
    };

    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        if let Err(e) = position_right_edge(&win) {
            eprintln!("[wikilens] failed to position overlay: {e}");
        }
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.emit(EVENT_SHOWN, ());
    }
}

/// Hide the overlay. Used by the `Esc` handler (via the `hide_overlay` command)
/// and the tray menu.
pub fn hide_overlay(app: &AppHandle) {
    if let Some(win) = overlay_window(app) {
        let _ = win.hide();
    }
}

/// Size the panel to full monitor height and dock it flush against the right
/// edge of whichever monitor currently hosts it.
///
/// Works in physical pixels throughout and adds the monitor's own offset, which
/// is what makes docking correct on multi-monitor and high-DPI setups.
pub fn position_right_edge(win: &WebviewWindow) -> tauri::Result<()> {
    let monitor = match win.current_monitor()? {
        Some(m) => m,
        None => match win.primary_monitor()? {
            Some(m) => m,
            None => return Ok(()), // No monitor info; leave the window where it is.
        },
    };

    let origin = monitor.position(); // physical top-left of this monitor
    let size = monitor.size(); // physical monitor resolution
    let scale = monitor.scale_factor();

    let panel_width = (PANEL_WIDTH as f64 * scale).round() as u32;
    win.set_size(PhysicalSize::new(panel_width, size.height))?;

    let x = origin.x + size.width as i32 - panel_width as i32;
    let y = origin.y;
    win.set_position(PhysicalPosition::new(x, y))?;

    Ok(())
}
