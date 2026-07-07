//! Overlay window control: toggle/show/hide and top-right float placement.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Window label from `tauri.conf.json`.
pub const OVERLAY_LABEL: &str = "overlay";
/// Logical panel width in CSS pixels. Physical width is scaled per-monitor.
pub const PANEL_WIDTH: u32 = 420;
/// Gap between the panel and the screen's top/right edges (CSS `--panel-gap`).
pub const PANEL_GAP: u32 = 12;
/// Extra window room left/bottom so the CSS drop shadow (`--shadow-panel:
/// 0 12px 32px`) renders instead of clipping at the window edge. Must match
/// `--shadow-room-left` / `--shadow-room-bottom` in styles.css.
pub const SHADOW_ROOM_LEFT: u32 = 32;
pub const SHADOW_ROOM_BOTTOM: u32 = 44;
/// Maximum panel height as a fraction of the monitor height (design
/// exploration 1a). The window is always sized for the cap; the CSS lets the
/// panel hug its content up to it.
pub const PANEL_HEIGHT_FRAC: f64 = 0.70;

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
        show_overlay(app);
    }
}

/// Dock the overlay to the top-right of its current monitor, show it, take
/// focus, and notify the frontend (which focuses the prompt). Extracted from
/// `toggle_overlay` so the capture flow can re-show the panel on teardown; the
/// re-shown panel auto-refocuses the prompt via the existing `EVENT_SHOWN`
/// handler.
pub fn show_overlay(app: &AppHandle) {
    let Some(win) = overlay_window(app) else {
        eprintln!("[wikilens] overlay window '{OVERLAY_LABEL}' not found");
        return;
    };
    if let Err(e) = position_top_right(&win) {
        eprintln!("[wikilens] failed to position overlay: {e}");
    }
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.emit(EVENT_SHOWN, ());
}

/// Hide the overlay. Used by the `Esc` handler (via the `hide_overlay` command)
/// and the tray menu.
pub fn hide_overlay(app: &AppHandle) {
    if let Some(win) = overlay_window(app) {
        let _ = win.hide();
    }
}

/// Size the window around the floating panel (70% of monitor height, gap at
/// top/right, shadow apron at left/bottom) and pin it to the top-right corner
/// of whichever monitor currently hosts it. The CSS margins in styles.css
/// carve the same gap/apron regions out of the webview, so the two must agree.
///
/// Works in physical pixels throughout and adds the monitor's own offset, which
/// is what makes placement correct on multi-monitor and high-DPI setups.
pub fn position_top_right(win: &WebviewWindow) -> tauri::Result<()> {
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
    let to_phys = |logical: u32| (logical as f64 * scale).round() as u32;

    let win_w = to_phys(SHADOW_ROOM_LEFT + PANEL_WIDTH + PANEL_GAP);
    let panel_h = (size.height as f64 * PANEL_HEIGHT_FRAC).round() as u32;
    let win_h = (panel_h + to_phys(PANEL_GAP + SHADOW_ROOM_BOTTOM)).min(size.height);
    win.set_size(PhysicalSize::new(win_w, win_h))?;

    let x = origin.x + size.width as i32 - win_w as i32;
    let y = origin.y;
    win.set_position(PhysicalPosition::new(x, y))?;

    Ok(())
}
