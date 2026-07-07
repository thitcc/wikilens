import type { ReactNode } from "react";

interface BadgeProps {
  children: ReactNode;
  /** Neutral by default; `accent` is reserved for rarer future badges — an
   * accent badge on the ~half of models that are vision-capable would drown
   * the accent selection check. */
  variant?: "neutral" | "accent";
  /** Hover text giving the badge context (e.g. what a capability means). */
  title?: string;
}

/** Small reusable capability pill. First consumer: an "Image" badge on
 * vision-capable model rows (see vault/2026-07-06_model-vision-badges.md).
 * Takes children (not a label prop) so future badges can pair an icon + text. */
export function Badge({ children, variant = "neutral", title }: BadgeProps) {
  return (
    <span className={"badge badge--" + variant} title={title}>
      {children}
    </span>
  );
}
