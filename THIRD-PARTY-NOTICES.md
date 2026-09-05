# Third-party notices

WikiLens itself is licensed under the MIT License (see `LICENSE`). The files
listed below are **excluded** from that grant. They are verbatim MediaWiki
pages captured as test data — compiled into the test build only
(`#[cfg(test)]` via `include_str!`), never into the shipped application — and
the four `.snap` files are machine-derived plaintext extracts of them.
Each remains under the license of the wiki it came from; the source URL and
capture date are also recorded in the first two lines of every fixture.

Historical revisions of this repository contain the same files.

## Shipped third-party code

The open-source components compiled into the application — every Rust crate
linked into `wikilens.exe` and every npm package bundled into the frontend —
are inventoried with their full license texts in `THIRD-PARTY-LICENSES.txt`
at the repository root. That file is generated (`npm run licenses`), checked
for freshness by CI, and installed next to `WikiLens.exe`.

## CC BY-SA 3.0 — Core Keeper Wiki (core-keeper.fandom.com)

Page: *Copper Ore*, captured 2026-07-13.

- `src-tauri/src/wiki/fixtures/fandom_portable_infobox.html`
- `src-tauri/src/wiki/snapshots/wikilens_lib__wiki__html__tests__fandom_portable_infobox.snap`

License: <https://creativecommons.org/licenses/by-sa/3.0/>

## CC BY-SA 2.5 — The Unofficial Elder Scrolls Pages (en.uesp.net)

Page: *Skyrim:Iron Ingot*, captured 2026-07-13.

- `src-tauri/src/wiki/fixtures/uesp_page.html`
- `src-tauri/src/wiki/snapshots/wikilens_lib__wiki__html__tests__uesp_skyrim_iron.snap`

License: <https://creativecommons.org/licenses/by-sa/2.5/>

## CC BY-NC-SA 3.0 — Stardew Valley Wiki (stardewvalleywiki.com)

**NonCommercial.** These four files may not be used for commercial purposes;
anyone redistributing this repository commercially must remove them.

Pages: *Parsnip* (rendered HTML) and *Wood* (raw wikitext), captured 2026-07-13.

- `src-tauri/src/wiki/fixtures/stardew_parsnip.html`
- `src-tauri/src/wiki/fixtures/raw_wikitext.txt`
- `src-tauri/src/wiki/snapshots/wikilens_lib__wiki__html__tests__stardew_parsnip.snap`
- `src-tauri/src/wiki/snapshots/wikilens_lib__wiki__wikitext__tests__raw_wikitext_stardew_wood.snap`

License: <https://creativecommons.org/licenses/by-nc-sa/3.0/>

## Runtime wiki content

At runtime WikiLens fetches pages from the wiki of the game you pick and
shows excerpts with links to their source pages. That content stays under
each wiki's own license (typically CC BY-SA) and is not part of this
repository.
