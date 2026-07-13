import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { AddGameMenu } from "./AddGameMenu";
import { GAMES, installBackend } from "../test/backend";

test("editing the name clears stale wiki candidates without re-probing", async () => {
  const backend = installBackend();
  const user = userEvent.setup();
  render(
    <AddGameMenu
      games={GAMES}
      onClose={() => {}}
      onAdded={() => {}}
      onRemoved={() => {}}
      triggerRef={{ current: null }}
    />,
  );

  await user.type(
    screen.getByRole("textbox", { name: "Game name" }),
    "Hollow Knight{Enter}",
  );
  await screen.findByRole("button", { name: /Hollow Knight Wiki/ });
  expect(backend.callsTo("suggest_wikis")).toHaveLength(1);

  // The suggestions were probed for the old name — keeping them would pair
  // the edited name with the wrong wiki on click.
  await user.type(screen.getByRole("textbox", { name: "Game name" }), "s");
  expect(screen.queryByRole("button", { name: /Hollow Knight Wiki/ })).toBeNull();
  expect(backend.callsTo("suggest_wikis")).toHaveLength(1);
});
