// Mount/interact helpers shared by the App-level tests.
//
// Suite rule — keyboard input goes through `userEvent` (`user.keyboard`,
// `user.type`), never a KeyboardEvent dispatched directly on `window`: a
// direct dispatch is AT_TARGET, where capture- and bubble-phase listeners run
// in registration order, silently inverting the layered-Esc behavior the
// menu tests exist to pin down.

import { act, render, screen } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import App from "../App";

/** Render <App/> (bare — no StrictMode, so IPC call counts stay
 * deterministic) and wait for the mount IPC to settle: both header and footer
 * chips label themselves from list_games / list_providers. */
export async function renderApp() {
  const result = render(<App />);
  await screen.findByRole("button", { name: /^Game: (?!Loading)/ });
  await screen.findByRole("button", { name: /^Model: / });
  return result;
}

/** Fire a backend event through the mocked event plugin (the app's real
 * listen() handlers run synchronously inside emit's invoke). */
export async function fireBackendEvent(
  event: string,
  payload?: unknown,
): Promise<void> {
  await act(async () => {
    await emit(event, payload);
  });
}
