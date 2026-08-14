//! Overlay window control: toggle/show/hide and anchored/manual placement.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::settings::{PanelAnchor, PanelPosition, PositionMode, SettingsStore};

/// Window label from `tauri.conf.json`.
pub const OVERLAY_LABEL: &str = "overlay";
/// Logical panel width in CSS pixels. Physical width is scaled per-monitor.
pub const PANEL_WIDTH: u32 = 420;
/// Gap between the panel and the screen edges it anchors to (the visual gap;
/// the window overhangs the screen by `apron − gap` on anchored edges so the
/// panel keeps it).
pub const PANEL_GAP: u32 = 12;
/// Shadow apron: extra window room on every side so the CSS drop shadow
/// (`--shadow-panel: 0 12px 32px`) renders instead of clipping at the window
/// edge. Symmetric because a draggable window has no screen edge to hide a
/// clipped shadow behind (the debug window's rationale) — bottom carries the
/// shadow's downward offset, top only its upward blur reach. Must match
/// `--shadow-room-*` in styles.css.
pub const APRON_TOP: u32 = 20;
pub const APRON_RIGHT: u32 = 32;
pub const APRON_BOTTOM: u32 = 44;
pub const APRON_LEFT: u32 = 32;
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
    (panel + to_phys(APRON_TOP + APRON_BOTTOM)).min(monitor_height)
}

/// Drag bookkeeping for the overlay, managed in `lib.rs` beside
/// `OverlayHeight`.
///
/// `applied` is the last outer position `apply_rect` handed the OS, recorded
/// BEFORE `set_position` because WM_MOVE can dispatch synchronously inside
/// the call — a `Moved` matching it is our own echo. The guard is
/// load-bearing: bottom/center anchors move `y` on every height report, so
/// without it a streaming answer would read as a drag and flip the mode to
/// Manual. `drag` is the in-flight override: set on the FIRST foreign Moved
/// (flipping effective placement to Manual before the debounced persist can
/// run, so a mid-drag height report can't snap the window back), cleared
/// when a Position pick makes the store authoritative again. `generation`
/// coalesces the debounce tasks.
#[derive(Default)]
pub struct DragTracker {
    applied: Mutex<Option<(i32, i32)>>,
    drag: Mutex<Option<(i32, i32)>>,
    generation: AtomicU64,
}

impl DragTracker {
    fn set_applied(&self, pos: (i32, i32)) {
        *self.applied.lock().unwrap_or_else(PoisonError::into_inner) = Some(pos);
    }

    fn is_applied(&self, pos: (i32, i32)) -> bool {
        *self.applied.lock().unwrap_or_else(PoisonError::into_inner) == Some(pos)
    }

    fn drag(&self) -> Option<(i32, i32)> {
        *self.drag.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Record a drag position; returns the new generation for the debounce.
    fn set_drag(&self, pos: (i32, i32)) -> u64 {
        *self.drag.lock().unwrap_or_else(PoisonError::into_inner) = Some(pos);
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Invalidate pending debounce tasks without recording a drag.
    fn bump(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn clear(&self) {
        *self.drag.lock().unwrap_or_else(PoisonError::into_inner) = None;
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
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

/// Event emitted to the overlay webview when Rust itself changes the stored
/// placement (a drag settled into Manual) — the stepper and the root data
/// attributes track the flip with the menu closed. Payload: `PositionInfo`
/// (mode/anchor/locked, never coordinates; pinned with `SettingsInfo`).
pub const EVENT_POSITION: &str = "settings://position";

/// Debounce before persisting a drag (or snapping a locked window back):
/// `Moved` fires per-frame during a drag, and writing settings — or calling
/// `set_position` — inside the OS move loop would churn or fight it.
const DRAG_SETTLE: Duration = Duration::from_millis(500);

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
    if let Err(e) = layout_overlay(&win) {
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
    if let Err(e) = layout_overlay(&win) {
        eprintln!("[wikilens] failed to lay out overlay: {e}");
    }
}

/// A Position pick makes the store authoritative again: drop the drag
/// override and invalidate any pending debounce task (a stale drag persist
/// firing after an anchor pick would overwrite it).
pub fn clear_drag_override(app: &AppHandle) {
    if let Some(tracker) = app.try_state::<DragTracker>() {
        tracker.clear();
    }
}

/// The overlay moved (`WindowEvent::Moved`, routed from lib.rs). Our own
/// `apply_rect` writes are filtered by the applied-target guard; anything
/// else while visible is a user drag — or, with the padlock on, a foreign/OS
/// move to undo.
pub fn on_overlay_moved(app: &AppHandle, pos: (i32, i32)) {
    let Some(tracker) = app.try_state::<DragTracker>() else {
        return;
    };
    if tracker.is_applied(pos) {
        return;
    }
    let Some(win) = overlay_window(app) else {
        return;
    };
    if !win.is_visible().unwrap_or(false) {
        return;
    }
    let Some(settings) = app.try_state::<SettingsStore>() else {
        return;
    };

    if settings.panel_position().locked {
        // Locked never flips the mode: snap back to the stored placement
        // once the move settles. Defense in depth — the header offers no
        // drag region while locked, so only foreign/OS moves land here.
        let generation = tracker.bump();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(DRAG_SETTLE).await;
            let Some(tracker) = app.try_state::<DragTracker>() else {
                return;
            };
            if tracker.generation() == generation {
                apply_layout(&app);
            }
        });
        return;
    }

    // Effective placement flips to Manual NOW (the override) — the store
    // catches up after the settle, and the anchor memory rides along
    // untouched, so stepping back to it still restores.
    let generation = tracker.set_drag(pos);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(DRAG_SETTLE).await;
        let Some(tracker) = app.try_state::<DragTracker>() else {
            return;
        };
        let Some(settings) = app.try_state::<SettingsStore>() else {
            return;
        };
        if tracker.generation() != generation {
            return; // superseded by more drag, or by a Position pick
        }
        let current = settings.panel_position();
        let next = PanelPosition {
            mode: PositionMode::Manual,
            manual: Some(pos),
            ..current
        };
        match settings.set_panel_position(next) {
            Ok(()) => {
                let _ = app.emit_to(
                    OVERLAY_LABEL,
                    EVENT_POSITION,
                    crate::commands::position_info(&settings),
                );
            }
            // Keep the session override: the panel stays where dragged and
            // the stepper shows the stale value until a successful save —
            // never yank the window back over a disk error.
            Err(e) => eprintln!("[wikilens] couldn't persist the dragged position: {e}"),
        }
    });
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

/// One physical rect (a monitor, the window, the header strip) for the pure
/// placement math.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

/// Where the layout math is told to put the window.
enum Placement {
    Anchor(PanelAnchor),
    /// The stored manual spot: the window's outer top-left, physical px.
    Manual(i32, i32),
}

/// How much of the panel's top strip must sit on a monitor for a stored
/// manual spot to count as grabbable (logical px): roughly the header's
/// height, and enough width to catch with a cursor.
const HEADER_GRAB_HEIGHT: u32 = 40;
const MIN_GRAB_WIDTH: u32 = 100;

/// The window rect for a placement on one monitor — pure math, physical px.
///
/// Anchored: the *panel* (the glass box inside the aprons) keeps `PANEL_GAP`
/// from the anchored screen edges by letting the outer apron hang off-screen
/// (`apron − gap` overhang) — the on-screen dead zone at an anchored corner
/// stays what it was before the symmetric apron. Center accepts the panel
/// riding 12 logical px above true center (the (bottom − top) apron
/// asymmetry, halved) — the offset is uniform across hug/cap heights, so
/// nothing jumps as the panel grows.
///
/// Growth (the height report changing the window height) falls out of
/// recomputing per call: top anchors keep the top edge (grow down), bottom
/// anchors keep the bottom edge (grow up), center keeps the window center.
/// Manual keeps the stored top-left and clamps the height to the room below
/// it on the hosting monitor, floored so a pathological spot can't collapse
/// the window.
fn overlay_rect(monitor: Rect, scale: f64, placement: Placement, desired: Option<u32>) -> Rect {
    let to_phys = |logical: u32| (logical as f64 * scale).round() as u32;
    let win_w = to_phys(APRON_LEFT + PANEL_WIDTH + APRON_RIGHT);
    let win_h = overlay_window_height(monitor.h, scale, desired);
    match placement {
        Placement::Anchor(anchor) => {
            let x = match anchor {
                PanelAnchor::TopRight | PanelAnchor::BottomRight => {
                    monitor.x + monitor.w as i32 - win_w as i32
                        + to_phys(APRON_RIGHT - PANEL_GAP) as i32
                }
                PanelAnchor::TopLeft | PanelAnchor::BottomLeft => {
                    monitor.x - to_phys(APRON_LEFT - PANEL_GAP) as i32
                }
                PanelAnchor::Center => monitor.x + (monitor.w as i32 - win_w as i32) / 2,
            };
            let y = match anchor {
                PanelAnchor::TopRight | PanelAnchor::TopLeft => {
                    monitor.y - to_phys(APRON_TOP - PANEL_GAP) as i32
                }
                PanelAnchor::BottomRight | PanelAnchor::BottomLeft => {
                    monitor.y + monitor.h as i32 - win_h as i32
                        + to_phys(APRON_BOTTOM - PANEL_GAP) as i32
                }
                PanelAnchor::Center => monitor.y + (monitor.h as i32 - win_h as i32) / 2,
            };
            Rect {
                x,
                y,
                w: win_w,
                h: win_h,
            }
        }
        Placement::Manual(x, y) => {
            // The panel may reach the monitor's bottom edge — only the apron
            // hangs off past it.
            let room_below =
                (monitor.y + monitor.h as i32 + to_phys(APRON_BOTTOM) as i32 - y).max(0) as u32;
            let floor = to_phys(MIN_PANEL_HEIGHT + APRON_TOP + APRON_BOTTOM);
            Rect {
                x,
                y,
                w: win_w,
                h: win_h.min(room_below).max(floor),
            }
        }
    }
}

/// The monitor a stored manual spot lays out on: the one showing the largest
/// slice of the panel's header strip, provided at least `min_grab_w` of the
/// strip — at its full height — is visible there. `None` (monitor unplugged
/// or rearranged) sends the caller back to the remembered anchor for this
/// show, deliberately WITHOUT rewriting settings: docking changes are often
/// transient, and the spot restores when the monitor returns.
fn manual_host(strip: Rect, min_grab_w: u32, monitors: &[Rect]) -> Option<usize> {
    let mut best: Option<(usize, u64)> = None;
    for (i, m) in monitors.iter().enumerate() {
        let w = overlap(strip.x, strip.w, m.x, m.w);
        let h = overlap(strip.y, strip.h, m.y, m.h);
        if w >= min_grab_w && h == strip.h {
            let area = u64::from(w) * u64::from(h);
            if best.is_none_or(|(_, b)| area > b) {
                best = Some((i, area));
            }
        }
    }
    best.map(|(i, _)| i)
}

/// Length of the overlap of `[a, a+aw)` and `[b, b+bw)`.
fn overlap(a: i32, aw: u32, b: i32, bw: u32) -> u32 {
    let lo = a.max(b);
    let hi = (a + aw as i32).min(b + bw as i32);
    (hi - lo).max(0) as u32
}

/// Lay the overlay out from the stored placement — the one sizer AND
/// positioner. `show_overlay` and `set_overlay_height` both route here, so a
/// height report never moves an edge the placement pins, and an anchored
/// panel always snaps back to its corner. The CSS margins in styles.css carve
/// the same apron regions out of the webview, so the two must agree.
///
/// Works in physical pixels throughout and adds the monitor's own offset,
/// which is what makes placement correct on multi-monitor and high-DPI
/// setups.
pub fn layout_overlay(win: &WebviewWindow) -> tauri::Result<()> {
    let app = win.app_handle();
    let position = app
        .try_state::<SettingsStore>()
        .map(|s| s.panel_position())
        .unwrap_or_default();
    let desired = app.try_state::<OverlayHeight>().and_then(|s| s.desired());

    // An in-flight drag owns the position: follow it as Manual, but apply
    // SIZE only — a set_position here would fight the OS move loop, and the
    // drag's own Moved stream keeps the override current.
    if let Some((x, y)) = app.try_state::<DragTracker>().and_then(|t| t.drag()) {
        let monitor = match win.current_monitor()? {
            Some(m) => m,
            None => match win.primary_monitor()? {
                Some(m) => m,
                None => return Ok(()),
            },
        };
        let rect = overlay_rect(
            Rect {
                x: monitor.position().x,
                y: monitor.position().y,
                w: monitor.size().width,
                h: monitor.size().height,
            },
            monitor.scale_factor(),
            Placement::Manual(x, y),
            desired,
        );
        let size = PhysicalSize::new(rect.w, rect.h);
        if win.outer_size()? != size {
            win.set_size(size)?;
        }
        return Ok(());
    }

    // Manual: lay out on the monitor hosting the stored spot, provided the
    // header is still grabbable there.
    if position.mode == PositionMode::Manual {
        if let Some((x, y)) = position.manual {
            // The window's current scale approximates the strip's own — fine
            // for a visibility probe (mixed-DPI drift is a few px).
            let scale = win.scale_factor()?;
            let to_phys = |logical: u32| (logical as f64 * scale).round() as u32;
            let strip = Rect {
                x: x + to_phys(APRON_LEFT) as i32,
                y: y + to_phys(APRON_TOP) as i32,
                w: to_phys(PANEL_WIDTH),
                h: to_phys(HEADER_GRAB_HEIGHT),
            };
            let monitors = win.available_monitors()?;
            let rects: Vec<Rect> = monitors
                .iter()
                .map(|m| Rect {
                    x: m.position().x,
                    y: m.position().y,
                    w: m.size().width,
                    h: m.size().height,
                })
                .collect();
            if let Some(i) = manual_host(strip, to_phys(MIN_GRAB_WIDTH), &rects) {
                let rect = overlay_rect(
                    rects[i],
                    monitors[i].scale_factor(),
                    Placement::Manual(x, y),
                    desired,
                );
                return apply_rect(win, rect);
            }
            eprintln!("[wikilens] stored manual spot is off every monitor; anchoring this show");
        }
        // Manual with nothing stored (or an unreachable spot) lays out from
        // the remembered anchor.
    }

    let monitor = match win.current_monitor()? {
        Some(m) => m,
        None => match win.primary_monitor()? {
            Some(m) => m,
            None => return Ok(()), // No monitor info; leave the window where it is.
        },
    };
    let rect = overlay_rect(
        Rect {
            x: monitor.position().x,
            y: monitor.position().y,
            w: monitor.size().width,
            h: monitor.size().height,
        },
        monitor.scale_factor(),
        Placement::Anchor(position.anchor),
        desired,
    );
    apply_rect(win, rect)
}

/// Apply a computed rect, skipping the OS round trip when the geometry is
/// already right — an answer streaming past the cap re-reports growing
/// logical values that all clamp to the same physical size. Deduping HERE
/// (against the real window) instead of at the store means a report after a
/// failed resize still retries.
fn apply_rect(win: &WebviewWindow, rect: Rect) -> tauri::Result<()> {
    // Record BEFORE set_position: WM_MOVE can dispatch synchronously inside
    // it, and the Moved handler must already know this position is ours.
    if let Some(tracker) = win.app_handle().try_state::<DragTracker>() {
        tracker.set_applied((rect.x, rect.y));
    }
    let target_size = PhysicalSize::new(rect.w, rect.h);
    let target_pos = PhysicalPosition::new(rect.x, rect.y);
    if win.outer_size()? == target_size && win.outer_position()? == target_pos {
        return Ok(());
    }
    win.set_size(target_size)?;
    win.set_position(target_pos)?;
    Ok(())
}

/// Store the frontend's height report and resize the window in place through
/// `layout_overlay` (the one sizer — each placement pins its own edge, so a
/// height report never moves the panel; it no-ops against the window's real
/// geometry, so repeated reports are cheap). Fine while hidden: the next show
/// re-derives anyway.
/// A NaN degrades safely (`as u32` saturates to 0 → "no report" → the cap).
pub fn set_overlay_height(app: &AppHandle, height: f64) {
    let Some(state) = app.try_state::<OverlayHeight>() else {
        return;
    };
    state.set(height.clamp(MIN_PANEL_HEIGHT as f64, 100_000.0).round() as u32);
    let Some(win) = overlay_window(app) else {
        return;
    };
    if let Err(e) = layout_overlay(&win) {
        eprintln!("[wikilens] failed to resize overlay: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_report_sizes_for_the_cap() {
        // 1080 * 0.70 = 756 panel + 64 apron chrome (top 20 + bottom 44).
        assert_eq!(overlay_window_height(1080, 1.0, None), 820);
    }

    #[test]
    fn a_short_report_hugs_the_panel() {
        assert_eq!(overlay_window_height(1080, 1.0, Some(220)), 284);
    }

    #[test]
    fn a_huge_report_clamps_to_the_cap() {
        // The menu-open sentinel path: the frontend never learns the cap.
        assert_eq!(overlay_window_height(1080, 1.0, Some(100_000)), 820);
    }

    #[test]
    fn dpi_scales_the_logical_report_with_ceil() {
        // ceil(401 * 1.5) = 602 panel + ceil(64 * 1.5) = 96 chrome. `round`
        // would grant 601 physical px for a 401px request — a fraction short,
        // re-creating 1px of .content overflow and a report ping-pong.
        assert_eq!(overlay_window_height(2160, 1.5, Some(401)), 698);
    }

    #[test]
    fn the_window_never_exceeds_the_monitor() {
        assert_eq!(overlay_window_height(100, 1.0, None), 100);
    }

    // ---- overlay_rect ------------------------------------------------------

    const FHD: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };

    fn anchored(monitor: Rect, scale: f64, anchor: PanelAnchor, desired: Option<u32>) -> Rect {
        overlay_rect(monitor, scale, Placement::Anchor(anchor), desired)
    }

    /// Every anchor at scale 1.0: window 484×820 (panel 420 + 32/32 aprons;
    /// cap 756 + 64 chrome). The ±20 x / −8 top / +32 bottom offsets are the
    /// aprons hanging off-screen so the panel keeps its 12px visual gap.
    #[test]
    fn anchors_place_exactly_at_scale_1() {
        for (anchor, x, y) in [
            (PanelAnchor::TopRight, 1920 - 484 + 20, -8),
            (PanelAnchor::TopLeft, -20, -8),
            (PanelAnchor::BottomRight, 1920 - 484 + 20, 1080 - 820 + 32),
            (PanelAnchor::BottomLeft, -20, 1080 - 820 + 32),
            (PanelAnchor::Center, (1920 - 484) / 2, (1080 - 820) / 2),
        ] {
            assert_eq!(
                anchored(FHD, 1.0, anchor, None),
                Rect {
                    x,
                    y,
                    w: 484,
                    h: 820
                },
                "{anchor:?}"
            );
        }
    }

    /// A 150% monitor with a virtual-desktop offset: every term scales and the
    /// monitor origin rides along. Window 726×1608 (round(484·1.5); 1512 cap +
    /// ceil(96) chrome).
    #[test]
    fn anchors_scale_and_offset_with_the_monitor() {
        let mon = Rect {
            x: 2560,
            y: -200,
            w: 3840,
            h: 2160,
        };
        for (anchor, x, y) in [
            (PanelAnchor::TopRight, 2560 + 3840 - 726 + 30, -200 - 12),
            (PanelAnchor::BottomLeft, 2560 - 30, -200 + 2160 - 1608 + 48),
            (PanelAnchor::Center, 2560 + (3840 - 726) / 2, -200 + (2160 - 1608) / 2),
        ] {
            assert_eq!(
                anchored(mon, 1.5, anchor, None),
                Rect {
                    x,
                    y,
                    w: 726,
                    h: 1608
                },
                "{anchor:?}"
            );
        }
    }

    /// Growth invariants: the edge a placement pins must not move when the
    /// height report changes — bottom anchors keep the bottom edge, center
    /// keeps the center, top anchors keep the top.
    #[test]
    fn growth_direction_pins_the_anchored_edge() {
        let tall = anchored(FHD, 1.0, PanelAnchor::BottomRight, None);
        let short = anchored(FHD, 1.0, PanelAnchor::BottomRight, Some(220));
        assert_eq!(tall.y + tall.h as i32, short.y + short.h as i32);

        let tall = anchored(FHD, 1.0, PanelAnchor::Center, None);
        let short = anchored(FHD, 1.0, PanelAnchor::Center, Some(220));
        assert_eq!(
            tall.y + tall.h as i32 / 2,
            short.y + short.h as i32 / 2,
            "center must stay centered as the panel grows"
        );

        let tall = anchored(FHD, 1.0, PanelAnchor::TopLeft, None);
        let short = anchored(FHD, 1.0, PanelAnchor::TopLeft, Some(220));
        assert_eq!(tall.y, short.y);
    }

    #[test]
    fn manual_keeps_the_spot_and_clamps_to_the_room_below() {
        // Plenty of room: the stored spot and the capped height, untouched.
        assert_eq!(
            overlay_rect(FHD, 1.0, Placement::Manual(100, 200), None),
            Rect {
                x: 100,
                y: 200,
                w: 484,
                h: 820
            }
        );
        // Near the bottom: the height gives way (panel may reach the monitor
        // edge; only the apron hangs off) down to the floor.
        assert_eq!(
            overlay_rect(FHD, 1.0, Placement::Manual(100, 1000), None).h,
            184, // floor: MIN_PANEL_HEIGHT 120 + 64 chrome
        );
        assert_eq!(
            overlay_rect(FHD, 1.0, Placement::Manual(100, 900), Some(220)).h,
            224, // room below (1080 + 44 − 900) wins over the 284 report
        );
    }

    // ---- manual_host -------------------------------------------------------

    /// The header strip a stored spot needs visible: built like
    /// `layout_overlay` builds it at scale 1.0.
    fn strip_at(x: i32, y: i32) -> Rect {
        Rect {
            x: x + APRON_LEFT as i32,
            y: y + APRON_TOP as i32,
            w: PANEL_WIDTH,
            h: HEADER_GRAB_HEIGHT,
        }
    }

    #[test]
    fn a_spot_on_a_monitor_finds_its_host() {
        assert_eq!(manual_host(strip_at(100, 200), MIN_GRAB_WIDTH, &[FHD]), Some(0));
    }

    #[test]
    fn a_spot_off_every_monitor_finds_none() {
        // The monitor that hosted the spot was unplugged.
        assert_eq!(
            manual_host(strip_at(2500, 200), MIN_GRAB_WIDTH, &[FHD]),
            None
        );
        assert_eq!(manual_host(strip_at(100, 200), MIN_GRAB_WIDTH, &[]), None);
    }

    #[test]
    fn a_header_above_the_screen_top_is_not_grabbable() {
        // Wide overlap, but the strip pokes above the monitor — the header
        // can't be caught with the cursor, so the anchor takes this show.
        assert_eq!(
            manual_host(strip_at(100, -30), MIN_GRAB_WIDTH, &[FHD]),
            None
        );
    }

    #[test]
    fn a_straddling_spot_prefers_the_monitor_showing_more_header() {
        let right_of_fhd = Rect {
            x: 1920,
            y: 0,
            w: 1920,
            h: 1080,
        };
        // Panel at x=1600: strip spans 1632..2052 — 288px on the first
        // monitor, 132px on the second. Both beat MIN_GRAB_WIDTH; the first
        // shows more.
        assert_eq!(
            manual_host(strip_at(1600, 200), MIN_GRAB_WIDTH, &[FHD, right_of_fhd]),
            Some(0)
        );
        // Panel at x=1850: 38px left / 382px right — only the second
        // clears the minimum.
        assert_eq!(
            manual_host(strip_at(1850, 200), MIN_GRAB_WIDTH, &[FHD, right_of_fhd]),
            Some(1)
        );
    }
}
