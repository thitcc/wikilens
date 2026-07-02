import type { Source } from "../types";
import { openExternal } from "../api";

interface SourceListProps {
  sources: Source[];
}

/** The wiki pages an answer drew from, opened in the system browser on click. */
export function SourceList({ sources }: SourceListProps) {
  if (sources.length === 0) return null;

  return (
    <div className="sources">
      <div className="sources-label">Sources</div>
      <ul>
        {sources.map((source) => (
          <li key={source.url}>
            <button
              type="button"
              className="source-link"
              title={source.url}
              onClick={() => void openExternal(source.url)}
            >
              {source.title}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
