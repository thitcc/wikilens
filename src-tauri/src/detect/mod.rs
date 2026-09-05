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
mod rules;

pub use foreground::probe;
pub use rules::RULES;

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

/// One foreground pattern that identifies a game.
///
/// All `&'static str`, so the table is a zero-allocation `&[DetectRule]` and
/// the type is `Copy`. Every field is lowercase — [`reduce`] pre-lowercases
/// the path leaves, so matching is plain `==` rather than a per-comparison
/// case fold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectRule {
    /// The game id this rule resolves to, as registered in `wiki::games`.
    pub game_id: &'static str,
    /// Executable basename **with** its extension.
    ///
    /// Mandatory, which makes a title-only rule unrepresentable by
    /// construction — such a rule would fire on a browser tab open at the
    /// game's own wiki.
    pub exe: &'static str,
    /// Required parent-directory leaf.
    ///
    /// The only thing that separates games shipping identical basenames:
    /// Path of Exile 1 and 2 both launch `PathOfExileSteam.exe` and differ
    /// solely by install directory. Note the leaf names the *game* only when
    /// the binary sits in the install root — Unreal titles bury theirs under
    /// `Binaries\Win64`, so their leaf is `win64` and this field is useless
    /// for them (see the plan's Documented gaps).
    pub parent: Option<&'static str>,
    /// Required case-insensitive title **prefix** — never `contains`, because
    /// "path of exile" is a substring of "Path of Exile 2".
    ///
    /// Only for executables shared with non-game software: `javaw.exe` is
    /// every Java application on the machine.
    pub title_prefix: Option<&'static str>,
}

impl DetectRule {
    /// Does this rule claim `fg`? `lower_title` is the caption lowercased once
    /// by the caller rather than per rule.
    fn claims(&self, fg: &Foreground, lower_title: &str) -> bool {
        self.exe == fg.exe
            && self.parent.is_none_or(|p| fg.parent.as_deref() == Some(p))
            && self.title_prefix.is_none_or(|t| lower_title.starts_with(t))
    }

    /// How many narrowing constraints the rule carries. A constrained rule
    /// outranks a bare one for the same executable, so adding a specific rule
    /// never collides with the general one it refines.
    fn specificity(&self) -> u8 {
        self.parent.is_some() as u8 + self.title_prefix.is_some() as u8
    }
}

/// Resolve a foreground window to a game id. Pure.
///
/// Deliberately **two-pass and order-independent**: table order is an accident
/// of editing and must never decide a match. Every uncertain case returns
/// `None` — unknown executable, unsatisfied constraint, or two equally
/// specific rules disagreeing. It never guesses: a miss costs a glance, a
/// wrong answer points the player at another game's wiki.
pub fn match_game(rules: &[DetectRule], fg: &Foreground) -> Option<&'static str> {
    let lower_title = fg.title.to_lowercase();

    // Pass 1: the most specific tier any claiming rule reaches.
    let best = rules
        .iter()
        .filter(|r| r.claims(fg, &lower_title))
        .map(DetectRule::specificity)
        .max()?;

    // Pass 2: that tier must name exactly one game. Several rules may claim
    // the window (a game can list many executables); several *games* may not.
    let mut winner: Option<&'static str> = None;
    for rule in rules
        .iter()
        .filter(|r| r.specificity() == best && r.claims(fg, &lower_title))
    {
        match winner {
            None => winner = Some(rule.game_id),
            Some(id) if id == rule.game_id => {}
            Some(_) => return None,
        }
    }
    winner
}

/// The whole pipeline: sample the foreground window, reduce it, match it
/// against the shipped rules. Every failure leg collapses to `None`, which
/// callers must treat as "keep whatever the player already picked" — never as
/// an error and never as "no game".
pub fn detect_foreground_game() -> Option<&'static str> {
    let (path, title) = probe()?;
    match_game(RULES, &reduce(&path, &title))
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
            r"C:\Users\x\AppData\Roaming\Some Game\game.exe",
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

    /// Match a raw path + caption the way a real summon would: reduce, then
    /// match against the shipped table.
    fn detect(path: &str, title: &str) -> Option<&'static str> {
        match_game(RULES, &reduce(path, title))
    }

    #[test]
    fn a_mixed_case_path_matches_the_lowercase_table() {
        // Case insensitivity is delivered by `reduce`, not by the comparison —
        // this pins that the two halves agree.
        assert_eq!(
            detect(r"D:\SteamLibrary\steamapps\common\Terraria\Terraria.exe", "Terraria"),
            Some("terraria")
        );
    }

    #[test]
    fn unknown_exe_matches_nothing() {
        assert_eq!(detect(r"C:\Windows\explorer.exe", "Program Manager"), None);
        assert_eq!(
            detect(
                r"C:\Users\x\AppData\Local\Programs\Microsoft VS Code\Code.exe",
                "wikilens - Visual Studio Code"
            ),
            None,
            "a non-game must never move the chip"
        );
    }

    #[test]
    fn poe_and_poe2_split_on_the_parent_directory() {
        // THE regression for the ship-blocker: both games launch the identical
        // basename, so the install directory is the only discriminator. If this
        // fails, PoE2 players get PoE1 answers with genuine-looking sources.
        let steam = r"C:\Program Files (x86)\Steam\steamapps\common";
        assert_eq!(
            detect(&format!(r"{steam}\Path of Exile\PathOfExileSteam.exe"), "Path of Exile"),
            Some("poe")
        );
        assert_eq!(
            detect(&format!(r"{steam}\Path of Exile 2\PathOfExileSteam.exe"), "Path of Exile 2"),
            Some("poe2")
        );
        // No directory to read: refuse rather than pick a side.
        assert_eq!(detect("PathOfExileSteam.exe", "Path of Exile"), None);
    }

    #[test]
    fn a_title_constrained_rule_needs_its_title() {
        let java = r"C:\Program Files\Java\jdk-21\bin\javaw.exe";
        assert_eq!(detect(java, "Minecraft 1.21.4 - Singleplayer"), Some("minecraft"));
        // Both mod loaders rewrite the caption; only the bare prefix survives.
        assert_eq!(detect(java, "Minecraft* Forge 1.21.1"), Some("minecraft"));
        assert_eq!(detect(java, "Minecraft NeoForge* 1.21.1"), Some("minecraft"));
        // Same host executable, different application.
        assert_eq!(detect(java, "IntelliJ IDEA"), None);
        assert_eq!(detect(java, ""), None);
        // The exe half is load-bearing too: the launcher is not the game.
        assert_eq!(
            detect(r"C:\XboxGames\Minecraft Launcher\Minecraft.exe", "Minecraft Launcher"),
            None
        );
    }

    #[test]
    fn title_prefix_is_a_prefix_not_a_substring() {
        // A `contains` rewrite would make "Path of Exile 2" match a "path of
        // exile" rule. Pin the semantics on the matcher directly.
        let rules = [DetectRule {
            game_id: "poe",
            exe: "game.exe",
            parent: None,
            title_prefix: Some("path of exile"),
        }];
        let fg = reduce("game.exe", "A guide to Path of Exile");
        assert_eq!(match_game(&rules, &fg), None);
    }

    #[test]
    fn a_constrained_rule_outranks_a_bare_rule() {
        // Adding a specific rule must refine a general one, not collide with it.
        let rules = [
            DetectRule { game_id: "general", exe: "game.exe", parent: None, title_prefix: None },
            DetectRule {
                game_id: "specific",
                exe: "game.exe",
                parent: Some("special"),
                title_prefix: None,
            },
        ];
        assert_eq!(match_game(&rules, &reduce(r"C:\special\game.exe", "")), Some("specific"));
        assert_eq!(match_game(&rules, &reduce(r"C:\other\game.exe", "")), Some("general"));
    }

    #[test]
    fn ambiguous_top_tier_matches_resolve_to_none() {
        // Two games claiming the same window equally well: never guess.
        let rules = [
            DetectRule { game_id: "one", exe: "game.exe", parent: None, title_prefix: None },
            DetectRule { game_id: "two", exe: "game.exe", parent: None, title_prefix: None },
        ];
        assert_eq!(match_game(&rules, &reduce(r"C:\games\game.exe", "")), None);
    }

    #[test]
    fn several_exes_for_one_game_are_not_ambiguous() {
        // Skyrim and GW2 each list multiple basenames; that must resolve, not
        // trip the ambiguity guard.
        assert_eq!(detect(r"C:\Games\Skyrim Special Edition\SkyrimSE.exe", ""), Some("skyrim"));
        assert_eq!(detect(r"C:\Games\Skyrim\TESV.exe", ""), Some("skyrim"));
        assert_eq!(detect(r"C:\Games\Guild Wars 2\Gw2-64.exe", ""), Some("gw2"));
    }

    #[test]
    fn match_is_order_independent() {
        // Table order is an accident of editing. This is what stops a future
        // "first match wins" rewrite from passing the rest of the suite.
        let reversed: Vec<DetectRule> = RULES.iter().rev().copied().collect();
        let cases = [
            (r"C:\g\Path of Exile 2\PathOfExileSteam.exe", "Path of Exile 2"),
            (r"C:\g\Path of Exile\PathOfExileSteam.exe", "Path of Exile"),
            (r"C:\g\Grounded\Maine\Binaries\Win64\Maine-Win64-Shipping.exe", "Grounded"),
            (r"C:\Java\bin\javaw.exe", "Minecraft 1.21"),
            (r"C:\Windows\explorer.exe", "Program Manager"),
        ];
        for (path, title) in cases {
            let fg = reduce(path, title);
            assert_eq!(
                match_game(RULES, &fg),
                match_game(&reversed, &fg),
                "table order changed the verdict for {path:?}"
            );
        }
    }

    #[test]
    fn grounded_matches_the_real_on_device_reading() {
        // Verbatim from the 2026-08-05 spike, minus the user's home directory.
        // Also pins that an Unreal game's parent leaf ("win64") is irrelevant
        // to a rule that does not constrain on it.
        let fg = reduce(
            r"C:\Users\x\Documents\Games\Grounded\Maine\Binaries\Win64\Maine-Win64-Shipping.exe",
            "Grounded",
        );
        assert_eq!(fg.parent.as_deref(), Some("win64"));
        assert_eq!(match_game(RULES, &fg), Some("grounded"));
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
