import type { RefObject } from "react";

interface PromptInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  disabled?: boolean;
  inputRef: RefObject<HTMLTextAreaElement | null>;
}

/**
 * Multi-line question box. Enter submits; Shift+Enter inserts a newline.
 * The ref lets the parent focus/select the field when the overlay opens.
 */
export function PromptInput({
  value,
  onChange,
  onSubmit,
  disabled,
  inputRef,
}: PromptInputProps) {
  return (
    <textarea
      ref={inputRef}
      className="prompt-input"
      value={value}
      disabled={disabled}
      rows={2}
      placeholder="Ask about the game… (Enter to send, Shift+Enter for a new line)"
      aria-label="Question"
      onChange={(e) => onChange(e.currentTarget.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          if (!disabled) onSubmit();
        }
      }}
    />
  );
}
