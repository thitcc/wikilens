//! Which executable belongs to which built-in game.
//!
//! Every field is lowercase — `detect::reduce` pre-lowercases the path leaves,
//! so matching is plain `==`.
//!
//! **Shipping gate for a new row, both halves required.** (i) The basename is
//! *distinctive*: it appears in no other rule and is not a generic host, a
//! launcher, or a bootstrap shim. A wrong distinctive name is a silent miss
//! and costs nothing; a wrong *shared* name points the player at another
//! game's wiki, which is the one outcome this table exists to prevent.
//! (ii) Either the on-device spike confirmed it, or ≥2 independent sources
//! agree and it carries no collision. Rows meeting only (ii) say so inline.
//!
//! **A game does not need a rule.** The offline test below runs one direction
//! only — every rule names a registered game — deliberately *not* its twin.
//! Requiring a rule per game would turn "add a wiki" from a four-line PR into
//! "buy the game", and the pressure release for that is an invented
//! executable name, i.e. exactly the failure the gate above prevents. A game
//! without a rule is fully functional; it just stays a manual pick.
//!
//! **Never in this table** — launchers and Unreal bootstrap shims
//! (`FuncomLauncher.exe`, Warframe's `Launcher.exe`, `SkyrimSELauncher.exe`,
//! `Fallout4Launcher.exe`, `AbioticFactor.exe`, `Grounded.exe`,
//! `MinecraftLauncher.exe`, `gamelaunchhelper.exe`); loaders
//! (`skse64_loader.exe`, `f4se_loader.exe`, `ModOrganizer.exe`,
//! `Vortex.exe`); supervisors and servers (`ConanSandbox_BE.exe`,
//! `TerrariaServer.exe`, `*Server-Win64-Shipping.exe`); generic hosts (bare
//! `dotnet.exe`, and `javaw.exe` without its title constraint); and dead or
//! fabricated names (`Warframe.exe` — 32-bit, end-of-life 2019 —, `KZW.exe`,
//! `SkyrimSE_original.exe`, `PathOfExile*_x64*.exe` and `Client.exe`, which
//! Grinding Gear Games confirms are backwards-compatibility stubs,
//! `Grounded2-Win64-Shipping.exe`, `tModLoader.exe`).
//!
//! Provenance and the per-game reasoning live in
//! `vault/2026-08-04_game-auto-detection.md`.

use super::DetectRule;

/// A game identified by its executable alone.
const fn exe(game_id: &'static str, exe: &'static str) -> DetectRule {
    DetectRule {
        game_id,
        exe,
        parent: None,
        title_prefix: None,
    }
}

/// A game whose executable is shared with another game, split by the folder
/// it launches from.
const fn in_dir(game_id: &'static str, exe: &'static str, parent: &'static str) -> DetectRule {
    DetectRule {
        game_id,
        exe,
        parent: Some(parent),
        title_prefix: None,
    }
}

/// A game running under a host executable shared with non-game software,
/// narrowed by the window caption.
const fn titled(game_id: &'static str, exe: &'static str, title_prefix: &'static str) -> DetectRule {
    DetectRule {
        game_id,
        exe,
        parent: None,
        title_prefix: Some(title_prefix),
    }
}

pub static RULES: &[DetectRule] = &[
    // SMAPI relaunches the game in its own process, and modded players are
    // told to copy it over "Stardew Valley.exe" on Game Pass. Never AND the
    // exe with a title here: SMAPI owns a *second* window in the same process
    // captioned "SMAPI …", so a title gate would drop detection whenever its
    // console has focus.
    exe("stardew", "stardew valley.exe"),
    exe("stardew", "stardewmoddingapi.exe"),
    exe("corekeeper", "corekeeper.exe"),
    // Renamed in the Enhanced update (2026-05-05, UE4→UE5). The pre-Enhanced
    // "ConanSandbox.exe" is still a selectable Steam branch but only
    // secondary-sourced, so it stays out until the spike sees it.
    // researched, distinctive, unconfirmed on-device
    exe("conanexiles", "conansandbox-win64-shipping.exe"),
    // 32-bit "Warframe.exe" is deliberately absent: end-of-support shipped
    // 2019-02-27 and a live-service client force-updates, so no player can be
    // running it. A dead basename is pure false-positive surface.
    exe("warframe", "warframe.x64.exe"),
    // The best case in the table: one binary is installer, updater, loader and
    // client, so detection fires from the patcher screen onward. "Gw2.exe" is
    // legacy-install only.
    exe("gw2", "gw2-64.exe"),
    exe("gw2", "gw2.exe"),
    // THE SHIP-BLOCKER. Path of Exile 1 and 2 declare byte-identical Steam
    // launch entries (`PathOfExileSteam.exe --nopatch`, appinfo 238960 and
    // 2694490); `config.installdir` is the only difference. Dropping `parent`
    // from either block silently routes PoE2 questions to the PoE1 wiki —
    // `poe_and_poe2_split_on_the_parent_directory` in mod.rs is the guard.
    in_dir("poe", "pathofexilesteam.exe", "path of exile"),
    in_dir("poe", "pathofexile.exe", "path of exile"),
    in_dir("poe", "pathofexileegs.exe", "path of exile"),
    in_dir("poe", "pathofexile_kg.exe", "path of exile"),
    in_dir("poe2", "pathofexilesteam.exe", "path of exile 2"),
    in_dir("poe2", "pathofexile.exe", "path of exile 2"),
    in_dir("poe2", "pathofexileegs.exe", "path of exile 2"),
    in_dir("poe2", "pathofexile_kg.exe", "path of exile 2"),
    // The Game Pass "AbioticFactor-WinGDK-Shipping.exe" has a documented
    // WinGDK config folder but only one observation lineage — it fails the
    // two-sources half of the gate and waits for the spike.
    // researched, distinctive, unconfirmed on-device
    exe("abioticfactor", "abioticfactor-win64-shipping.exe"),
    exe("davethediver", "davethediver.exe"),
    // SKSE launches the runtime and owns no window of its own, so every launch
    // route (Steam, skse64_loader, MO2, Vortex) lands on these. Game Pass ships
    // the same basenames.
    exe("skyrim", "skyrimse.exe"),
    exe("skyrim", "tesv.exe"),
    exe("skyrim", "skyrimvr.exe"),
    exe("skyrim", "tesv_original.exe"),
    exe("fallout4", "fallout4.exe"),
    exe("fallout4", "fallout4vr.exe"),
    // Project codename "Maine" — underivable from the game name, which is why
    // this needed research at all. Confirmed on-device 2026-08-05.
    exe("grounded", "maine-win64-shipping.exe"),
    exe("grounded", "maine-wingdk-shipping.exe"),
    // Platform tag "WinGRTS" (Steam) / "WinGDK" (Xbox); no Win64 build exists.
    // Shares no prefix with Grounded 1, which is what keeps the two pointed at
    // their two different wikis.
    exe("grounded2", "grounded2-wingrts-shipping.exe"),
    exe("grounded2", "grounded2-wingdk-shipping.exe"),
    // Vanilla only. tModLoader 1.4 runs as `dotnet.exe` from a `dotnet`
    // subfolder, so neither the basename nor the parent leaf can reach it —
    // covering it needs a path-contains-segment rule, a deliberate later
    // extension. Terraria also has no usable title rule at all: the caption is
    // a randomly re-rolled, localised, mod-rewritable title message.
    exe("terraria", "terraria.exe"),
    // The one place a title constraint is load-bearing rather than a tiebreak:
    // java hosts every Java application on the machine. Only the bare prefix
    // survives the mod loaders — Forge renders "Minecraft* Forge 1.21.x" and
    // NeoForge "Minecraft NeoForge* 1.21.x", so any tighter pattern breaks
    // both. The exe half is equally load-bearing: the Xbox-app Java launcher's
    // window is titled "Minecraft Launcher".
    titled("minecraft", "javaw.exe", "minecraft"),
    titled("minecraft", "java.exe", "minecraft"),
    // Bedrock has been a GDK app since 1.21.120 and owns an ordinary top-level
    // window, which is why no ApplicationFrameHost child-walk is needed.
    exe("minecraft", "minecraft.windows.exe"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::games;

    #[test]
    fn every_rule_targets_a_builtin_game() {
        // One direction only, on purpose — see this module's header for why the
        // twin (every game needs a rule) would be actively harmful.
        for rule in RULES {
            assert!(
                games::find_game(rule.game_id).is_some(),
                "rule for {:?} names an unregistered game id {:?}",
                rule.exe,
                rule.game_id
            );
        }
    }

    #[test]
    fn rule_exes_are_lowercase_and_end_in_exe() {
        // `reduce` hands the matcher a lowercased basename, so an uppercase
        // table entry would simply never fire. The `.exe` check catches a full
        // path pasted in by mistake.
        for rule in RULES {
            assert_eq!(rule.exe, rule.exe.to_lowercase(), "rule exe must be lowercase");
            assert!(
                rule.exe.ends_with(".exe"),
                "rule exe {:?} is not a basename",
                rule.exe
            );
            assert!(
                !rule.exe.contains('\\') && !rule.exe.contains('/'),
                "rule exe {:?} carries a path",
                rule.exe
            );
        }
    }

    #[test]
    fn rule_constraints_are_lowercase() {
        for rule in RULES {
            for constraint in [rule.parent, rule.title_prefix].into_iter().flatten() {
                assert_eq!(
                    constraint,
                    constraint.to_lowercase(),
                    "constraint {constraint:?} must be lowercase to ever match"
                );
            }
        }
    }

    #[test]
    fn no_rule_constrains_on_a_generic_platform_folder() {
        // The parent leaf names the *game* only when the binary sits in the
        // install root — the shape Path of Exile has, and the reason the
        // poe/poe2 split works. Unreal titles bury theirs under
        // `Binaries\Win64`, so their leaf is `win64`, shared with every other
        // UE game; .NET apps launch from a `dotnet` folder. Confirmed
        // on-device 2026-08-05: Grounded reduces to parent `win64`.
        //
        // A parent constraint on one of these either never fires or fires for
        // the wrong game. Covering such a title needs a path-contains-segment
        // rule, which this type deliberately does not have.
        const GENERIC: &[&str] = &[
            "win64", "win32", "wingdk", "wingrts", "binaries", "bin", "x64", "x86", "dotnet",
        ];
        for rule in RULES {
            if let Some(parent) = rule.parent {
                assert!(
                    !GENERIC.contains(&parent),
                    "rule for {:?} constrains on {parent:?}, a platform folder shared \
                     across games — it cannot identify one",
                    rule.exe
                );
            }
        }
    }

    #[test]
    fn unconstrained_rules_do_not_share_an_exe() {
        // Two bare rules on one executable sit in the same specificity tier, so
        // the matcher would call it ambiguous and BOTH games would become
        // permanently undetectable — silently. Constrained duplicates are fine
        // and expected (that is the poe/poe2 shape).
        let bare: Vec<_> = RULES.iter().filter(|r| r.specificity() == 0).collect();
        for (i, a) in bare.iter().enumerate() {
            for b in &bare[i + 1..] {
                assert!(
                    a.exe != b.exe || a.game_id == b.game_id,
                    "{:?} is claimed unconditionally by both {:?} and {:?}",
                    a.exe,
                    a.game_id,
                    b.game_id
                );
            }
        }
    }
}
