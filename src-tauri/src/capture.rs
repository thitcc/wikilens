//! In-game screenshot capture.
//!
//! Flow: `begin` freezes the monitor under the cursor into a `PendingShot`
//! (an `RgbaImage` snapshot + that monitor's physical origin) and shows the
//! capture overlay; the capture webview lets the user drag a region and reports
//! it as a `CropRect`; `finish` runs the pure, unit-tested `crop_to_attachment`
//! to produce an `Attachment` (cropped, downscaled, PNG-encoded).
//!
//! The full PNG never crosses IPC — only a small thumbnail data-URI does
//! (`AttachmentInfo`). It lives in `AppState` until the next successful `ask`
//! consumes it (or it is cleared), matching the "all image bytes stay in Rust"
//! rule the LLM/wiki HTTP already follows.

use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use base64::Engine as _;
use image::{imageops, RgbaImage};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

use crate::error::AppError;
use crate::state::AppState;

/// Capture window label from `tauri.conf.json`.
pub const CAPTURE_LABEL: &str = "capture";

/// Longest edge (px) of an attached image. Vision models downscale anything
/// larger anyway (Anthropic caps the long edge at ~1568 px); trimming here
/// keeps the base64 payload — and the input-token bill — small.
const MAX_EDGE: u32 = 1568;

/// Longest edge (px) of the UI thumbnail. Small enough that its data-URI stays
/// ~10–30 KB across IPC.
const THUMB_EDGE: u32 = 88;

/// Minimum selection size (physical px) below which a drag is treated as a
/// misclick. The capture webview also guards this in CSS px; this is the
/// Rust-side backstop the pure crop function enforces.
const MIN_DRAG: u32 = 4;

/// How long to wait after hiding the overlay before snapshotting, so the
/// compositor has actually removed the panel from the screen.
const SETTLE: Duration = Duration::from_millis(120);

/// Monotonic id source. A capture's id lets `ask` reject a stale attachment
/// (one captured, then replaced) instead of sending the wrong image.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> String {
    format!("shot-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

/// Poison-tolerant lock, matching `AppState`'s model-cache discipline: the
/// guard is only ever held for a synchronous section, never across an await.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A monitor snapshot frozen at trigger time. Held between `begin` and
/// `finish`; encoding is deferred to `finish` so the drag stays instant.
pub struct PendingShot {
    /// Full monitor image at physical resolution.
    image: RgbaImage,
    /// Physical top-left of that monitor on the virtual desktop (negative on a
    /// left/secondary monitor). Positions the capture window over it.
    monitor_x: i32,
    monitor_y: i32,
}

/// A finished attachment: the encoded PNG plus its identity and pixel size.
/// Held in `AppState` until a successful `ask` consumes it or it is cleared.
pub struct Attachment {
    pub id: String,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl Attachment {
    /// The IPC-facing summary: id, pixel size, and a small thumbnail data-URI
    /// — never the full PNG. Built by decoding the stored PNG and re-encoding a
    /// thumbnail; if any step fails (it just encoded cleanly, so it shouldn't),
    /// it falls back to the full PNG so the user still sees their capture.
    pub fn to_info(&self) -> AttachmentInfo {
        let thumb = image::load_from_memory(&self.png)
            .ok()
            .map(|img| downscale(img.to_rgba8(), THUMB_EDGE))
            .and_then(encode_png);
        let bytes: &[u8] = thumb.as_deref().unwrap_or(&self.png);
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        AttachmentInfo {
            id: self.id.clone(),
            thumb_uri: format!("data:image/png;base64,{b64}"),
            width: self.width,
            height: self.height,
        }
    }
}

/// What crosses IPC after a capture: the id the frontend echoes back on `ask`,
/// a small thumbnail data-URI for the prompt strip, and the pixel size.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInfo {
    pub id: String,
    pub thumb_uri: String,
    pub width: u32,
    pub height: u32,
}

/// The drag rectangle from the capture webview: CSS pixels relative to the
/// window's top-left (which sits at the monitor origin), plus the window's
/// `devicePixelRatio`. Deserialized from the `finish_capture` command payload.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub dpr: f64,
}

/// Freeze the monitor under the cursor and show the capture overlay.
///
/// Resolves the cursor's monitor *before* hiding the panel (the pointer must
/// not have moved), hides the overlay so the user can see and select what is
/// beneath it, waits a beat for the compositor, then snapshots and positions
/// the capture window over exactly that monitor. Any failure after the panel
/// is hidden restores it, so a capture error never leaves the user with a blank
/// screen.
pub async fn begin(app: &AppHandle, state: &AppState) -> Result<(), AppError> {
    if lock(&state.pending_shot).is_some() {
        return Err(AppError::Capture("A capture is already in progress.".into()));
    }

    // Global cursor position, physical px (negatives allowed) — the exact
    // coordinate space xcap's `from_point` (MonitorFromPoint) expects.
    let cursor = app
        .cursor_position()
        .map_err(|e| AppError::Capture(format!("Couldn't read the cursor position: {e}")))?;
    let (cx, cy) = (cursor.x.round() as i32, cursor.y.round() as i32);

    crate::window::hide_overlay(app);
    tokio::time::sleep(SETTLE).await;

    let result = begin_after_hide(app, state, cx, cy).await;
    if result.is_err() {
        // Bring the panel back so the error is visible and the app is usable.
        crate::window::show_overlay(app);
    }
    result
}

/// The part of `begin` that runs while the overlay is hidden; split out so the
/// caller can restore the overlay on any error here.
async fn begin_after_hide(
    app: &AppHandle,
    state: &AppState,
    cx: i32,
    cy: i32,
) -> Result<(), AppError> {
    // xcap's capture is blocking — keep it off the async runtime's poll thread.
    let shot = tauri::async_runtime::spawn_blocking(move || capture_monitor_at(cx, cy))
        .await
        .map_err(|e| AppError::Capture(format!("The capture task failed to run: {e}")))??;

    let win = app
        .get_webview_window(CAPTURE_LABEL)
        .ok_or_else(|| AppError::Capture("The capture window is missing.".into()))?;
    let (w, h) = (shot.image.width(), shot.image.height());
    win.set_position(PhysicalPosition::new(shot.monitor_x, shot.monitor_y))
        .and_then(|()| win.set_size(PhysicalSize::new(w, h)))
        .map_err(|e| AppError::Capture(format!("Couldn't place the capture overlay: {e}")))?;

    *lock(&state.pending_shot) = Some(shot);
    let _ = win.show();
    let _ = win.set_focus();
    // The capture webview is pre-declared and never reloaded, so tell it a new
    // capture is live and it should re-arm its handlers (see src/capture/main.ts).
    let _ = win.emit("capture://armed", ());
    Ok(())
}

/// Blocking: snapshot the monitor containing the given physical point.
fn capture_monitor_at(x: i32, y: i32) -> Result<PendingShot, AppError> {
    let monitor = xcap::Monitor::from_point(x, y)
        .map_err(|e| AppError::Capture(format!("Couldn't find the monitor under the cursor: {e}")))?;
    let monitor_x = monitor.x().map_err(geometry_err)?;
    let monitor_y = monitor.y().map_err(geometry_err)?;
    let image = monitor
        .capture_image()
        .map_err(|e| AppError::Capture(format!("Couldn't capture the screen: {e}")))?;
    Ok(PendingShot {
        image,
        monitor_x,
        monitor_y,
    })
}

fn geometry_err(e: xcap::XCapError) -> AppError {
    AppError::Capture(format!("Couldn't read the monitor geometry: {e}"))
}

/// Consume the pending shot, crop it to `rect`, and store the result as the
/// current attachment. Always tears down the capture UI and restores the
/// overlay first, so the panel comes back whether the crop succeeds or not.
pub fn finish(app: &AppHandle, state: &AppState, rect: CropRect) -> Result<AttachmentInfo, AppError> {
    let pending = lock(&state.pending_shot).take();
    close_capture_ui(app, state);

    let shot = pending.ok_or_else(|| AppError::Capture("No capture is in progress.".into()))?;
    match crop_to_attachment(&shot.image, rect, next_id()) {
        Some(attachment) => {
            let info = attachment.to_info();
            *lock(&state.attachment) = Some(attachment);
            Ok(info)
        }
        None => Err(AppError::Capture(
            "That selection was too small — drag a larger box.".into(),
        )),
    }
}

/// Cancel an in-progress capture: tear down the UI and restore the overlay.
/// Any already-attached image is left untouched.
pub fn cancel(app: &AppHandle, state: &AppState) {
    close_capture_ui(app, state);
}

/// Drop the current attachment (the "×" remove, and after a successful `ask`).
pub fn clear(state: &AppState) {
    *lock(&state.attachment) = None;
}

/// The stored attachment's PNG bytes when its id matches `image_id`.
///
/// `Ok(None)` when no image was requested; `Err` when an id was requested but
/// doesn't match what's stored (a stale/replaced capture) — surfaced to the
/// user as "capture it again" rather than silently sending the wrong image.
/// Clones the PNG so no lock is held across the LLM await.
pub fn resolve_image(state: &AppState, image_id: Option<&str>) -> Result<Option<Vec<u8>>, AppError> {
    let Some(id) = image_id else {
        return Ok(None);
    };
    let guard = lock(&state.attachment);
    match guard.as_ref() {
        Some(att) if att.id == id => Ok(Some(att.png.clone())),
        _ => Err(AppError::Capture(
            "That screenshot expired — capture it again.".into(),
        )),
    }
}

/// Hide the capture window, drop any pending shot, and bring the overlay back.
fn close_capture_ui(app: &AppHandle, state: &AppState) {
    if let Some(win) = app.get_webview_window(CAPTURE_LABEL) {
        let _ = win.hide();
    }
    *lock(&state.pending_shot) = None;
    crate::window::show_overlay(app);
}

/// Crop `rect` out of a monitor snapshot, downscale to `MAX_EDGE`, and
/// PNG-encode. **Pure** — the DPI math lives here so tests pin it: physical
/// px = round(css × dpr), the box is normalized (right-to-left / bottom-to-top
/// drags) and clamped to the image (an over-drag at the edge can't panic).
/// Returns `None` when the clamped selection is below `MIN_DRAG` on either axis.
fn crop_to_attachment(image: &RgbaImage, rect: CropRect, id: String) -> Option<Attachment> {
    let (iw, ih) = (image.width(), image.height());
    let dpr = if rect.dpr > 0.0 { rect.dpr } else { 1.0 };

    // Normalize the drag direction, then convert CSS px → physical px.
    let left = rect.x.min(rect.x + rect.w) * dpr;
    let top = rect.y.min(rect.y + rect.h) * dpr;
    let right = rect.x.max(rect.x + rect.w) * dpr;
    let bottom = rect.y.max(rect.y + rect.h) * dpr;

    // Clamp to image bounds via i32 so a negative pre-clamp value can't wrap.
    let x0 = (left.round() as i32).clamp(0, iw as i32) as u32;
    let y0 = (top.round() as i32).clamp(0, ih as i32) as u32;
    let x1 = (right.round() as i32).clamp(0, iw as i32) as u32;
    let y1 = (bottom.round() as i32).clamp(0, ih as i32) as u32;

    let cw = x1.saturating_sub(x0);
    let ch = y1.saturating_sub(y0);
    if cw < MIN_DRAG || ch < MIN_DRAG {
        return None;
    }

    let cropped = imageops::crop_imm(image, x0, y0, cw, ch).to_image();
    let scaled = downscale(cropped, MAX_EDGE);
    let (width, height) = (scaled.width(), scaled.height());
    let png = encode_png(scaled)?;
    Some(Attachment {
        id,
        png,
        width,
        height,
    })
}

/// Downscale so the longest edge is ≤ `max_edge`; returned unchanged when
/// already within bounds (never upscales).
fn downscale(image: RgbaImage, max_edge: u32) -> RgbaImage {
    let (w, h) = (image.width(), image.height());
    let longest = w.max(h);
    if longest <= max_edge {
        return image;
    }
    let scale = max_edge as f64 / longest as f64;
    let nw = ((w as f64 * scale).round() as u32).max(1);
    let nh = ((h as f64 * scale).round() as u32).max(1);
    imageops::resize(&image, nw, nh, imageops::FilterType::Triangle)
}

/// PNG-encode an RGBA image. `None` on the (practically impossible) encode
/// failure, so the caller can degrade rather than unwrap-panic.
fn encode_png(image: RgbaImage) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .ok()?;
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]))
    }

    fn rect(x: f64, y: f64, w: f64, h: f64, dpr: f64) -> CropRect {
        CropRect { x, y, w, h, dpr }
    }

    #[test]
    fn crop_at_dpr_1_matches_css_pixels() {
        let img = solid(200, 100);
        let att = crop_to_attachment(&img, rect(10.0, 10.0, 50.0, 40.0, 1.0), "id".into()).unwrap();
        assert_eq!((att.width, att.height), (50, 40));
    }

    #[test]
    fn crop_scales_css_by_dpr() {
        // 150% monitor: a 100×80 CSS selection is 150×120 physical px.
        let img = solid(300, 300);
        let att = crop_to_attachment(&img, rect(0.0, 0.0, 100.0, 80.0, 1.5), "id".into()).unwrap();
        assert_eq!((att.width, att.height), (150, 120));
    }

    #[test]
    fn negative_drag_is_normalized() {
        // Drag up-left from (100,90) by (-60,-50) → box (40,40)–(100,90).
        let img = solid(200, 200);
        let att =
            crop_to_attachment(&img, rect(100.0, 90.0, -60.0, -50.0, 1.0), "id".into()).unwrap();
        assert_eq!((att.width, att.height), (60, 50));
    }

    #[test]
    fn overdrag_is_clamped_to_image_bounds() {
        // Drag past the right/bottom edges → clamps to the remaining 20×20.
        let img = solid(120, 80);
        let att =
            crop_to_attachment(&img, rect(100.0, 60.0, 999.0, 999.0, 1.0), "id".into()).unwrap();
        assert_eq!((att.width, att.height), (20, 20));
    }

    #[test]
    fn tiny_drag_is_rejected() {
        let img = solid(50, 50);
        assert!(crop_to_attachment(&img, rect(10.0, 10.0, 3.0, 3.0, 1.0), "id".into()).is_none());
    }

    #[test]
    fn zero_dpr_is_treated_as_one() {
        // A malformed dpr must not collapse the selection to nothing.
        let img = solid(100, 100);
        let att = crop_to_attachment(&img, rect(0.0, 0.0, 40.0, 40.0, 0.0), "id".into()).unwrap();
        assert_eq!((att.width, att.height), (40, 40));
    }

    #[test]
    fn oversized_crop_is_downscaled_to_max_edge() {
        let img = solid(3000, 1500);
        let att =
            crop_to_attachment(&img, rect(0.0, 0.0, 3000.0, 1500.0, 1.0), "id".into()).unwrap();
        assert_eq!(att.width, MAX_EDGE);
        assert_eq!(att.height, 784); // 1500 × (1568/3000) = 783.99 → 784
                                     // Encoded output really is a PNG.
        assert_eq!(&att.png[1..4], b"PNG");
    }
}
