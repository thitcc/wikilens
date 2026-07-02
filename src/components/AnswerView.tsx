import Markdown from "react-markdown";
import { openExternal } from "../api";

interface AnswerViewProps {
  /** The streamed/finished markdown answer. */
  markdown: string;
}

/**
 * Renders the answer as markdown. `react-markdown` does not render raw HTML by
 * default, so model output is safe to display. Links are opened in the system
 * browser instead of navigating the webview.
 */
export function AnswerView({ markdown }: AnswerViewProps) {
  return (
    <div className="answer">
      <Markdown
        components={{
          a: ({ href, children }) => (
            <a
              href={href}
              onClick={(e) => {
                e.preventDefault();
                if (href) void openExternal(href);
              }}
            >
              {children}
            </a>
          ),
        }}
      >
        {markdown}
      </Markdown>
    </div>
  );
}
