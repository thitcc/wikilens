// The Micrographics theme's "instrument layer" — pure helpers for the
// component-voice marks that need JS (DESIGN.md §8). The governing split:
// JS owns characters (underscores), CSS owns case (text-transform under
// [data-theme="micrographics"]), so a label is never transformed twice and
// accessible names stay sentence case by construction.

import type { AskStatus } from "./types";

/** Visible-label treatment for chips under Micrographics: spaces become
 * underscores, case untouched (CSS uppercases). Applied to display text only —
 * never to aria-labels. */
export function instrumentLabel(text: string): string {
  return text.replace(/ /g, "_");
}

/** The prompt placeholder under Micrographics — CSS uppercases it to
 * ASK_ABOUT_THE_GAME…. Shorter than the default placeholder on purpose: the
 * key hints would read as shouting in caps, and the kbd chips below the
 * prompt already teach them. */
export const MICRO_PROMPT_PLACEHOLDER = "Ask_about_the_game…";

/** Dot-matrix progress: total dots in the row. */
export const DOT_TOTAL = 8;

/** Filled dots per phase. Monotone across the linear path
 * (searching → reading → answering); `understanding` and `retrying` share
 * searching's neighborhood because neither is a linear step (CLAUDE.md §4,
 * ask://status) — the row must never move backwards when they appear. Never 0
 * (the ask is running) and never 8 (the row unmounts on resolve). */
const DOT_FILL: Record<AskStatus, number> = {
  searching: 2,
  understanding: 3,
  retrying: 3,
  reading: 5,
  answering: 7,
};

/** How many of the DOT_TOTAL dots are lit for a phase. */
export function statusDots(status: AskStatus): number {
  return DOT_FILL[status];
}
