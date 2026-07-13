import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { AnswerView } from "./AnswerView";
import { installBackend } from "../test/backend";

test("markdown links open externally instead of navigating the webview", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  const href = "https://terraria.wiki.gg/wiki/Water_Candle";
  render(<AnswerView markdown={`See the [Water Candle](${href}) page.`} />);

  let clickEvent: MouseEvent | null = null;
  const capture = (e: MouseEvent) => {
    clickEvent = e;
  };
  document.addEventListener("click", capture);
  try {
    await user.click(screen.getByRole("link", { name: "Water Candle" }));
  } finally {
    document.removeEventListener("click", capture);
  }

  // preventDefault is the no-navigation oracle; the opener plugin got the URL.
  expect(clickEvent).not.toBeNull();
  expect(clickEvent!.defaultPrevented).toBe(true);
  const opens = backend.callsTo("plugin:opener|open_url") as Array<{
    url: string;
  }>;
  expect(opens).toHaveLength(1);
  expect(opens[0].url).toBe(href);
});
