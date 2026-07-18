// One ask's card: header, phase table with bars, three collapsible detail
// groups. Pure view over a DebugAskEntry — all ordering rules live in
// askState.ts.

import { useState, type ReactNode } from "react";

import type { DebugUsage } from "../types";
import { STATUS_LABELS, type DebugAskEntry } from "./askState";

/** Mirrors the stderr table's tokens row: `-/-` = the call was never made;
 * `?` per field = the call happened but the provider reported nothing. */
function usagePair(usage: DebugUsage | undefined): string {
  if (!usage) return "-/-";
  return `${usage.input ?? "?"}/${usage.output ?? "?"}`;
}

/** A collapsible detail section (the model menu's group-header idiom). */
function Group({
  title,
  note,
  children,
}: {
  title: string;
  note?: string;
  children: ReactNode;
}) {
  const [collapsed, setCollapsed] = useState(true);
  return (
    <div className="group">
      <button
        type="button"
        className="group-header"
        aria-expanded={!collapsed}
        onClick={() => setCollapsed((c) => !c)}
      >
        <span className={"group-caret" + (collapsed ? " is-collapsed" : "")}>
          ▾
        </span>
        {title}
        {note !== undefined && <span className="group-note">{note}</span>}
      </button>
      {!collapsed && <div className="group-body">{children}</div>}
    </div>
  );
}

export default function AskCard({ ask }: { ask: DebugAskEntry }) {
  const live = !ask.finished;
  // Relative bars: the slowest completed phase is the widest. Skipped phases
  // (elapsedMs null) render a muted dash instead of a bar.
  const slowest = Math.max(
    1,
    ...ask.phases.map((p) => p.elapsedMs ?? 0),
  );

  return (
    <article className={"ask-card" + (live ? " is-live" : "")}>
      <header className="ask-header">
        <div className="ask-question" title={ask.question}>
          {ask.question}
        </div>
        <div className="ask-meta">
          <span>{ask.game}</span>
          <span className="ask-model">{ask.answerModel}</span>
          {ask.rewriteModel !== null && (
            <span className="ask-model">rewrite: {ask.rewriteModel}</span>
          )}
        </div>
        <div className="ask-outcome">
          {ask.finished ? (
            <>
              <span
                className={
                  "outcome-badge" + (ask.finished.aborted ? " is-aborted" : "")
                }
              >
                {ask.finished.outcome}
              </span>
              <span className="ask-total">{ask.finished.totalMs} ms</span>
            </>
          ) : (
            <span className="outcome-badge is-running">running…</span>
          )}
        </div>
      </header>

      <div className="phases">
        {ask.phases.map((phase, i) => (
          <div className="phase-row" key={i}>
            <span className="phase-name">{phase.name}</span>
            {phase.elapsedMs === null ? (
              <span className="phase-bar phase-bar--skipped">—</span>
            ) : (
              <span className="phase-bar">
                <span
                  className="phase-bar-fill"
                  style={{ width: `${(phase.elapsedMs / slowest) * 100}%` }}
                />
              </span>
            )}
            <span className="phase-ms">
              {phase.elapsedMs === null ? "-" : phase.elapsedMs}
            </span>
            <span className="phase-detail" title={phase.detail}>
              {phase.detail}
            </span>
          </div>
        ))}
        {live && ask.lastStatus && (
          <div className="phase-row phase-row--provisional">
            <span className="phase-name">{STATUS_LABELS[ask.lastStatus]}</span>
            <span className="phase-bar">
              <span className="phase-bar-fill phase-bar-fill--running" />
            </span>
            <span className="phase-ms">…</span>
            <span className="phase-detail">running</span>
          </div>
        )}
      </div>

      <Group
        title="Queries & candidates"
        note={ask.candidates.length ? String(ask.candidates.length) : undefined}
      >
        <div className="detail-line">
          <span className="detail-label">query</span> {ask.query}
        </div>
        {ask.candidates.map((candidate, i) => (
          <div className="detail-line" key={i}>
            <span className="detail-label">cand {i + 1}</span> {candidate}
          </div>
        ))}
        {ask.candidates.length === 0 && (
          <div className="detail-line detail-line--muted">no candidates</div>
        )}
      </Group>

      <Group
        title="Pages"
        note={ask.pages.length ? String(ask.pages.length) : undefined}
      >
        {ask.pages.map((page) => (
          <div className="detail-line" key={page.title}>
            {page.title} <span className="detail-chars">({page.chars})</span>
          </div>
        ))}
        {ask.pages.length === 0 && (
          <div className="detail-line detail-line--muted">no pages fetched</div>
        )}
      </Group>

      <Group title="Tokens">
        <div className="detail-line">
          <span className="detail-label">rewrite in/out</span>{" "}
          {usagePair(ask.usage.rewrite)}
        </div>
        <div className="detail-line">
          <span className="detail-label">answer in/out</span>{" "}
          {usagePair(ask.usage.answer)}
        </div>
      </Group>
    </article>
  );
}
