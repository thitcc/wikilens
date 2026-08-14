//! Overlay window control: toggle/show/hide and top-right float placement.

use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;
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
/// exploration 1a). The window is sized for the frontend's reported panel
/// height clamped to this cap — no report yet, or a menu-open sentinel,
/// holds the cap. The window must hug the glass because the transparent
/// remainder still captures mouse input (Tauri transparent windows aren't
/// click-through): an idle panel over a full-cap window ate the game's
/// clicks (vault/2026-08-03_window-follows-panel-height.md).
pub const PANEL_HEIGHT_FRAC: f64 = 0.70;
/// Smallest panel height a report may request (logical px) — a degenerate
/// measurement must never collapse the window to just chrome.
pub const MIN_PANEL_HEIGHT: u32 = 120;

/// Frontend-reported desired panel height, logical CSS px, managed in
/// `lib.rs`. 0 = no report yet → size for the 70% cap (the safe default for
/// a first summon racing the webview's first report).
#[derive(Default)]
pub struct OverlayHeight(AtomicU32);

impl OverlayHeight {
    fn desired(&self) -> Option<u32> {
        match self.0.load(Ordering::SeqCst) {
            0 => None,
            h => Some(h),
        }
    }

    fn set(&self, height: u32) {
        self.0.store(height, Ordering::SeqCst);
    }
}

/// Physical window height for a reported logical panel height (`None` = no
/// report yet → the 70% cap). `ceil` on logical→physical so the granted CSS
/// room is never a fraction short of the request — rounding down would
/// recreate ~1px of `.content` overflow and ping-pong reports at 125%/150%
/// DPI scaling. (The chrome term also moved round→ceil: identical at every
/// standard Windows scale, at most +1 physical px of apron at odd custom
/// fractions — accepted.)
fn overlay_window_height(monitor_height: u32, scale: f64, desired: Option<u32>) -> u32 {
    let to_phys = |logical: u32| (logical as f64 * scale).ceil() as u32;
    let cap = (monitor_height as f64 * PANEL_HEIGHT_FRAC).round() as u32;
    let panel = desired.map_or(cap, |d| to_phys(d).min(cap));
    (panel + to_phys(PANEL_GAP + SHADOW_ROOM_BOTTOM)).min(monitor_height)
}

/// Event emitted after the panel is shown so the frontend can focus the input.
const EVENT_SHOWN: &str = "overlay://shown";

/// Payload of `overlay://shown`.
///
/// A struct rather than a bare `Option` so later summon-time facts join it
/// here instead of minting sibling events — Tauri gives no ordering guarantee
/// between separate emits, and the frontend's handler is timing-sensitive (it
/// arms the entrance animation).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShownInfo {
    /// The game the foreground window was identified as. `None` when nothing
    /// matched, when the foreground was one of our own windows, or when the
    /// process could not be read.
    ///
    /// A **suggestion**, never an instruction: the frontend offers it and only
    /// a click applies it. `None` means "unknown, keep the player's pick" —
    /// never "no game", and never an error.
    pub detected_game: Option<String>,
}
/// Event emitted after the panel is hidden. The frontend timestamps it to
/// suppress the next show's select-all when the hide was moments ago — the
/// guard born as the capital-C trap fix (the old Shift+C default meant typing
/// `C` fired the toggle; re-summon selected the draft, so the next keystroke
/// replaced the whole question). Kept for any accidental hide.
const EVENT_HIDDEN: &str = "overlay://hidden";

fn overlay_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(OVERLAY_LABEL)
}

/// Disable DWM's own transitions for this window
/// (`DWMWA_TRANSITIONS_FORCEDISABLED`). Windows plays a one-time fade + short
/// upward rise when an HWND is presented for the first time — our windows are
/// created hidden, so that lands on the first summon and layers OS motion
/// over A-01's right-edge slide; later hide/show cycles of the same HWND
/// never replay it, which is why only the first entrance looked different.
/// Must run before the window is first shown; called from `.setup()` for the
/// config windows and from `debug_window::create`. Best-effort: on failure
/// the cosmetic OS transition simply plays, so log-and-continue.
#[cfg(windows)]
pub fn disable_os_open_transition(win: &WebviewWindow) {
    use windows_sys::Win32::Foundation::TRUE;
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
    };

    let hwnd = match win.hwnd() {
        Ok(h) => h.0,
        Err(e) => {
            eprintln!(
                "[wikilens] no HWND for '{}', OS open transition stays on: {e}",
                win.label()
            );
            return;
        }
    };
    // Win32 BOOL is a plain i32 in windows-sys; the attribute id is typed
    // DWMWINDOWATTRIBUTE (i32) but the raw call takes u32.
    let disable: i32 = TRUE;
    let hr = unsafe {
        DwmSetWindowAttribute(
            hwnd as _,
            DWMWA_TRANSITIONS_FORCEDISABLED as u32,
            &disable as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        )
    };
    if hr != 0 {
        eprintln!(
            "[wikilens] DWM transition suppression failed for '{}': 0x{:08X}",
            win.label(),
            hr as u32
        );
    }
}

#[cfg(not(windows))]
pub fn disable_os_open_transition(_win: &WebviewWindow) {}

/// Toggle the overlay: hide if visible, otherwise dock it to the right edge,
/// show it, take focus, and notify the frontend.
pub fn toggle_overlay(app: &AppHandle) {
    let Some(win) = overlay_window(app) else {
        eprintln!("[wikilens] overlay window '{OVERLAY_LABEL}' not found");
        return;
    };

    if win.is_visible().unwrap_or(false) {
        hide_overlay(app);
    } else {
        show_overlay(app);
    }
}

/// Session tally behind the debug line. Diagnostic only — never crosses IPC.
/// The ratio is what says whether the rules table is worth growing; nobody has
/// estimated it, and between multi-monitor focus loss, launcher phases and
/// user-added wikis it may well be under half.
static DETECT_HITS: AtomicU32 = AtomicU32::new(0);
static DETECT_MISSES: AtomicU32 = AtomicU32::new(0);

/// Identify the game this summon is about to cover.
///
/// Must run **before** the panel is shown: `set_focus()` below makes the
/// overlay the foreground window, and a sample taken after it reads WikiLens
/// itself. Every `show_overlay` call site is pre-guarded to a hidden overlay,
/// so the PID check inside the probe is the only special case needed.
///
/// Only the resolved game id travels onward. The executable path and window
/// caption stay inside `detect` — never a `debug://` payload (whose key sets
/// are pin-tested), never `history.json`, never IPC.
fn detect_game_under_overlay() -> Option<String> {
    let detected = crate::detect::detect_foreground_game();
    if crate::debug::debug_enabled() {
        let hits = DETECT_HITS.load(Ordering::SeqCst);
        let misses = DETECT_MISSES.load(Ordering::SeqCst);
        match detected {
            Some(id) => eprintln!("[wikilens] detected {id} ({} hit / {misses} miss)", hits + 1),
            None => eprintln!("[wikilens] detected nothing ({hits} hit / {} miss)", misses + 1),
        }
    }
    let counter = if detected.is_some() {
        &DETECT_HITS
    } else {
        &DETECT_MISSES
    };
    counter.fetch_add(1, Ordering::SeqCst);
    detected.map(str::to_string)
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
    // Sample first: `set_focus()` below makes the overlay the foreground
    // window, so anything later would read WikiLens itself.
    let detected_game = detect_game_under_overlay();
    if let Err(e) = position_top_right(&win) {
        eprintln!("[wikilens] failed to position overlay: {e}");
    }
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.emit(EVENT_SHOWN, ShownInfo { detected_game });
}

/// Show the overlay only when it's hidden. The guard is the point: re-showing
/// a visible panel would re-emit `overlay://shown`, whose frontend handler
/// select-alls the prompt — on an open panel with a draft, the next keystroke
/// would replace the whole question. Used by the `show_overlay` command (the
/// capture hotkey's "this model can't read images" surface).
pub fn show_overlay_if_hidden(app: &AppHandle) {
    let Some(win) = overlay_window(app) else {
        eprintln!("[wikilens] overlay window '{OVERLAY_LABEL}' not found");
        return;
    };
    if !win.is_visible().unwrap_or(false) {
        show_overlay(app);
    }
}

/// The overlay's current outer position (physical virtual-screen px) — the
/// first Manual pick's "stay where you are" snapshot (`set_panel_position`).
pub fn overlay_outer_position(app: &AppHandle) -> Option<(i32, i32)> {
    let win = overlay_window(app)?;
    win.outer_position().ok().map(|p| (p.x, p.y))
}

/// Re-apply the stored placement to the live window — the `set_panel_position`
/// command routes here so a Position pick moves the panel immediately. Fine
/// while hidden (the next show re-derives anyway).
pub fn apply_layout(app: &AppHandle) {
    let Some(win) = overlay_window(app) else {
        return;
    };
    if let Err(e) = position_top_right(&win) {
        eprintln!("[wikilens] failed to lay out overlay: {e}");
    }
}

/// Hide the overlay and notify the frontend. Every hide path routes through
/// here (`Esc` via the `hide_overlay` command, the hotkey toggle, the tray,
/// Alt+F4, the capture flow) so `overlay://hidden` always fires — the
/// webview stays mounted while hidden and keeps listening.
pub fn hide_overlay(app: &AppHandle) {
    if let Some(win) = overlay_window(app) {
        let _ = win.hide();
        let _ = win.emit(EVENT_HIDDEN, ());
    }
}

/// Size the window around the floating panel (the reported panel height
/// capped at 70% of the monitor, gap at top/right, shadow apron at
/// left/bottom) and pin it to the top-right corner of whichever monitor
/// currently hosts it. The CSS margins in styles.css carve the same
/// gap/apron regions out of the webview, so the two must agree. The one
/// sizer: `set_overlay_height` routes through here too.
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
    let desired = win
        .app_handle()
        .try_state::<OverlayHeight>()
        .and_then(|s| s.desired());
    let win_h = overlay_window_height(size.height, scale, desired);
    win.set_size(PhysicalSize::new(win_w, win_h))?;

    let x = origin.x + size.width as i32 - win_w as i32;
    let y = origin.y;
    let target_size = PhysicalSize::new(win_w, win_h);
    let target_pos = PhysicalPosition::new(x, y);
    // Skip the OS round trip when the geometry is already right — an answer
    // streaming past the cap re-reports growing logical values that all
    // clamp to the same physical size. Deduping HERE (against the real
    // window) instead of at the store means a report after a failed resize
    // still retries.
    if win.outer_size()? == target_size && win.outer_position()? == target_pos {
        return Ok(());
    }
    win.set_size(target_size)?;
    win.set_position(target_pos)?;

    Ok(())
}

/// Store the frontend's height report and resize the window in place through
/// `position_top_right` (the one sizer — top-anchored, so height never moves
/// the panel; it no-ops against the window's real geometry, so repeated
/// reports are cheap). Fine while hidden: the next show re-derives anyway.
/// A NaN degrades safely (`as u32` saturates to 0 → "no report" → the cap).
pub fn set_overlay_height(app: &AppHandle, height: f64) {
    let Some(state) = app.try_state::<OverlayHeight>() else {
        return;
    };
    state.set(height.clamp(MIN_PANEL_HEIGHT as f64, 100_000.0).round() as u32);
    let Some(win) = overlay_window(app) else {
        return;
    };
    if let Err(e) = position_top_right(&win) {
        eprintln!("[wikilens] failed to resize overlay: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_report_sizes_for_the_cap() {
        // 1080 * 0.70 = 756 panel + 56 chrome — today's exact pre-hug size.
        assert_eq!(overlay_window_height(1080, 1.0, None), 812);
    }

    #[test]
    fn a_short_report_hugs_the_panel() {
        assert_eq!(overlay_window_height(1080, 1.0, Some(220)), 276);
    }

    #[test]
    fn a_huge_report_clamps_to_the_cap() {
        // The menu-open sentinel path: the frontend never learns the cap.
        assert_eq!(overlay_window_height(1080, 1.0, Some(100_000)), 812);
    }

    #[test]
    fn dpi_scales_the_logical_report_with_ceil() {
        // ceil(401 * 1.5) = 602 panel + ceil(56 * 1.5) = 84 chrome. `round`
        // would grant 601 physical px for a 401px request — a fraction short,
        // re-creating 1px of .content overflow and a report ping-pong.
        assert_eq!(overlay_window_height(2160, 1.5, Some(401)), 686);
    }

    #[test]
    fn the_window_never_exceeds_the_monitor() {
        assert_eq!(overlay_window_height(100, 1.0, None), 100);
    }
}
