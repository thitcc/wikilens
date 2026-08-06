// Foreground game detection: `overlay://shown` carries the game Rust found
// under the panel, and the header offers it as a one-tap chip.
//
// The load-bearing property of the whole feature is that detection never moves
// the selection on its own — a process-name match is an inference, and a wrong
// one must cost a glance, not an answer sourced from the wrong game's wiki.
// Most of this file exists to pin the cases where NOTHING should happen.

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import App from "./App";
import { deferred, GAMES, installBackend } from "./test/backend";
import { fireBackendEvent, renderApp } from "./test/harness";
import type { AskResult, GameInfo } from "./types";

/** The game chip's current label, via its accessible name. */
function gameChipName(): string | null {
  return screen
    .getByRole("button", { name: /^Game: / })
    .getAttribute("aria-label");
}

/** The suggestion chip, or null when nothing is being offered. */
function suggestion(): HTMLElement | null {
  return screen.queryByRole("button", { name: /^Switch to / });
}

/** Mount on the happy-path backend, then summon with a detection payload. */
async function summonDetecting(detectedGame: string | null) {
  installBackend();
  await renderApp();
  await fireBackendEvent("overlay://shown", { detectedGame });
}

test("a detected game is offered, not applied", async () => {
  // "terraria" is the mount-time selection (list_games[0]); detect the second.
  await summonDetecting("stardew-valley");

  expect(suggestion()?.textContent).toContain("Stardew Valley");
  // The selection itself is untouched until the player acts.
  expect(gameChipName()).toBe("Game: Terraria");
  expect(localStorage.getItem("wikilens.selectedGame")).toBeNull();
});

test("accepting the offer switches the game and persists it like a manual pick", async () => {
  const user = userEvent.setup();
  await summonDetecting("stardew-valley");

  await user.click(suggestion()!);

  expect(gameChipName()).toBe("Game: Stardew Valley");
  expect(localStorage.getItem("wikilens.selectedGame")).toBe("stardew-valley");
  // A tapped suggestion IS a manual pick, so it belongs in Recent.
  expect(
    JSON.parse(localStorage.getItem("wikilens.recentGames") ?? "[]"),
  ).toEqual(["stardew-valley"]);
  // Offer consumed: detected now equals selected, so there is nothing to show.
  expect(suggestion()).toBeNull();
  // Focus returns to the prompt — the accepted chip unmounts under the cursor,
  // and focus would otherwise drop to <body> and deaden the keyboard.
  expect(document.activeElement).toBe(
    screen.getByRole("textbox", { name: "Question" }),
  );
});

test("no detection changes nothing and shows nothing", async () => {
  // Summoned over a browser, the desktop, the tray, or one of our own windows.
  await summonDetecting(null);

  expect(suggestion()).toBeNull();
  expect(gameChipName()).toBe("Game: Terraria");
});

test("a detection matching the current game is not offered", async () => {
  await summonDetecting("terraria");

  expect(suggestion()).toBeNull();
  expect(gameChipName()).toBe("Game: Terraria");
});

test("an unknown detected id is ignored", async () => {
  // A removed user wiki, or a rules-table row for a game this build dropped.
  await summonDetecting("gone-game");

  expect(suggestion()).toBeNull();
  expect(gameChipName()).toBe("Game: Terraria");
});

test("a payload-less overlay://shown is tolerated", async () => {
  // Nothing emits bare today, but the normalization in api.ts is what keeps
  // every other suite's `fireBackendEvent("overlay://shown")` working — if it
  // regresses, this fails here instead of as a mystery elsewhere.
  installBackend();
  await renderApp();
  await fireBackendEvent("overlay://shown");

  expect(suggestion()).toBeNull();
  expect(gameChipName()).toBe("Game: Terraria");
});

test("nothing is offered while an ask is in flight", async () => {
  // Switching mid-stream would leave the chip disagreeing with the answer's
  // own sources. Reachable for real: submit, Esc, re-summon over another game.
  const user = userEvent.setup();
  const gate = deferred<AskResult>();
  installBackend({ ask: () => gate.promise });
  await renderApp();

  await user.type(
    screen.getByRole("textbox", { name: "Question" }),
    "best crops{Enter}",
  );
  await fireBackendEvent("overlay://shown", { detectedGame: "stardew-valley" });

  expect(suggestion()).toBeNull();

  // And it returns once the ask settles — the suppression is scoped to the
  // in-flight window, not a permanent dismissal.
  gate.resolve({ answer: "Kale.", sources: [] });
  await waitFor(() => expect(suggestion()).not.toBeNull());
});

test("a detection delivered before the games list resolves still gets offered", async () => {
  // The regression guard for the stale-closure hazard: onOverlayShown is
  // registered in a mount-only effect, so anything reading `games` inside that
  // closure sees []. Validating at render time is what makes this pass.
  // Mounted without renderApp()'s wait — that wait is exactly what this gate
  // prevents from resolving.
  const gate = deferred<GameInfo[]>();
  installBackend({ list_games: () => gate.promise });
  render(<App />);

  await fireBackendEvent("overlay://shown", { detectedGame: "stardew-valley" });
  expect(suggestion()).toBeNull(); // nothing to match against yet

  gate.resolve(GAMES);
  await screen.findByRole("button", { name: "Switch to Stardew Valley" });
});
