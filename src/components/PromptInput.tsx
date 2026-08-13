import type { RefObject } from "react";

interface PromptInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  busy?: boolean;
  /** Theme-specific placeholder override (the Micrographics instrument
   * layer); the component itself stays theme-unaware. */
  placeholder?: string;
  inputRef: RefObject<HTMLTextAreaElement | null>;
}

/**
 * Multi-line question box. Enter submits; Shift+Enter inserts a newline.
 * The ref lets the parent focus/select the field when the overlay opens.
 * While an ask runs the box is `readOnly`, not `disabled`: a disabled
 * textarea drops keyboard focus to `<body>`, so the follow-up question would
 * start typing into nothing. readOnly keeps focus and caret; keydown still
 * fires, so the `busy` guard below (and the submit handler's own) stays
 * load-bearing.
 */
export function PromptInput({
  value,
  onChange,
  onSubmit,
  busy,
  placeholder,
  inputRef,
}: PromptInputProps) {
  return (
    <textarea
      ref={inputRef}
      className="prompt-input"
      value={value}
      readOnly={busy}
      rows={2}
      placeholder={
        placeholder ??
        "Ask about the game… (Enter to send, Shift+Enter for a new line)"
      }
      aria-label="Question"
      onChange={(e) => onChange(e.currentTarget.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          if (!busy) onSubmit();
        }
      }}
    />
  );
}
