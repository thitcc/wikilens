//! Foreground-game detection: identify the game the overlay is covering so the
//! panel can offer its wiki instead of making the player pick every time.
//!
//! Split deliberately in two. `foreground.rs` is the Windows-only probe —
//! untestable FFI, no branching logic worth a test. Everything in this module
//! is pure and unit-tested, and `reduce` is the seam between them. It is also
//! the privacy boundary: see [`Foreground`].
//!
//! Phase 0 (vault/2026-08-04_game-auto-detection.md): the probe and the
//! reduction ship behind `WIKILENS_DEBUG` so the executables and captions of
//! real games can be observed on-device *before* any matching table is
//! written. A guessed table is the one failure this design exists to prevent —
//! a wrong match would silently answer from the wrong wiki. The rules table
//! and the matcher land in the follow-up, gated on those observations.

mod foreground;

pub use foreground::probe;

/// The foreground window reduced to matchable strings.
///
/// **Privacy boundary.** `QueryFullProcessImageNameW` hands back a full path
/// that embeds the user's install layout and often their username. Exactly two
/// lowercased leaves survive into this type — the executable's basename and its
/// parent folder — and [`reduce`] guarantees neither can contain a path
/// separator or a drive letter (pinned by `reduce_never_emits_a_path`). The
/// full path is never stored and never crosses IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Foreground {
    /// Executable basename including the extension, lowercased: `"gw2-64.exe"`.
    pub exe: String,
    /// The executable's parent directory leaf, lowercased: `"guild wars 2"`.
    /// `None` for a rootless path or one sitting directly on a drive root.
    ///
    /// This is the only thing that can separate two games shipping identical
    /// basenames, which is not hypothetical: Path of Exile 1 and 2 both launch
    /// `PathOfExileSteam.exe` and are distinguishable only by their install
    /// directory.
    pub parent: Option<String>,
    /// The window caption, verbatim. Deliberately *not* lowercased — the
    /// phase-0 spike needs to observe real captions, and title matching
    /// lowercases at compare time instead.
    pub title: String,
}

/// Reduce a full image path and window caption to the matchable form. Pure.
pub fn reduce(image_path: &str, title: &str) -> Foreground {
    // Both separators, because either can appear in a Win32 path.
    let mut segments = image_path.split(['\\', '/']).filter(|s| !s.is_empty()).rev();

    let exe = segments.next().unwrap_or_default().to_lowercase();
    // A drive specifier ("C:") is not a folder, and letting it through would
    // put a colon — a fragment of the real path — into the reduced form.
    let parent = segments
        .next()
        .filter(|s| !s.ends_with(':'))
        .map(str::to_lowercase);

    Foreground {
        exe,
        parent,
        title: title.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_splits_both_separators() {
        let back = reduce(
            r"C:\Program Files (x86)\Steam\steamapps\common\Guild Wars 2\Gw2-64.exe",
            "Guild Wars 2",
        );
        assert_eq!(back.exe, "gw2-64.exe");
        assert_eq!(back.parent.as_deref(), Some("guild wars 2"));

        // Forward slashes are legal in Win32 paths and do reach us.
        let fwd = reduce("C:/Games/Terraria/Terraria.exe", "Terraria");
        assert_eq!(fwd.exe, "terraria.exe");
        assert_eq!(fwd.parent.as_deref(), Some("terraria"));
    }

    #[test]
    fn reduce_never_emits_a_path() {
        // The structural privacy pin: whatever goes in, the two leaves that
        // come out cannot carry a directory, a drive, or a username. If this
        // ever fails, machine-identifying material is one refactor away from
        // crossing IPC.
        let cases = [
            r"C:\Users\Thiago\AppData\Roaming\Some Game\game.exe",
            r"C:\game.exe",
            r"\\fileserver\share\Games\Path of Exile\PathOfExileSteam.exe",
            "D:/steam/steamapps/common/Stardew Valley/Stardew Valley.exe",
            "game.exe",
            "",
        ];
        for path in cases {
            let fg = reduce(path, r"C:\not\a\path either");
            for leaf in [Some(&fg.exe), fg.parent.as_ref()].into_iter().flatten() {
                assert!(
                    !leaf.contains('\\') && !leaf.contains('/') && !leaf.contains(':'),
                    "reduce leaked path material from {path:?}: {leaf:?}"
                );
            }
        }
    }

    #[test]
    fn reduce_handles_a_rootless_path() {
        let fg = reduce("Terraria.exe", "Terraria");
        assert_eq!(fg.exe, "terraria.exe");
        assert_eq!(fg.parent, None, "a bare basename has no parent to report");
    }

    #[test]
    fn reduce_drive_root_parent_is_none() {
        let fg = reduce(r"C:\Terraria.exe", "Terraria");
        assert_eq!(fg.exe, "terraria.exe");
        assert_eq!(fg.parent, None, "\"C:\" is a drive specifier, not a folder");
    }

    #[test]
    fn reduce_lowercases_the_path_leaves_but_keeps_the_title() {
        // The asymmetry is deliberate and the matcher will depend on it: the
        // two path leaves arrive pre-lowercased so the rules table can be plain
        // lowercase strings compared with `==`, while the caption stays
        // verbatim for the spike to report.
        let fg = reduce(
            r"C:\Games\Path Of Exile 2\PathOfExileSteam.exe",
            "Path of Exile 2",
        );
        assert_eq!(fg.exe, "pathofexilesteam.exe");
        assert_eq!(fg.parent.as_deref(), Some("path of exile 2"));
        assert_eq!(fg.title, "Path of Exile 2");
    }

    #[test]
    fn probe_links_and_returns_without_panicking() {
        // The FFI's real behaviour is unverifiable here — CI has no game in the
        // foreground, and on a headless runner there may be no foreground
        // window at all. Either outcome is valid; what this pins is that the
        // symbols link, the signature matches, and neither leg panics.
        let _ = probe();
    }
}
