---
title: Explore short lowercase names with dry humor
type: plan
status: done
created: 2026-09-06
updated: 2026-09-06
tags: [overlay]
related: ["[[2026-07-02_scaffold]]"]
commit: b9626be
---

# Explore short lowercase names with dry humor

## Context / problem
The owner requested a naming shortlist with simple lowercase spelling, dry humor, and either Portuguese, English, or invented roots. `bizu` is a sound-and-personality reference. The product answers wiki questions over a running game, then disappears.

## Goal / non-goals
- Goal: present researched name candidates in a proof sheet using both current product themes, and record any owner preferences received during the exploration.
- Non-goal: rename the application, reserve domains, or assert trademark availability.

## Approach
1. Read the product register, current tokens, and proof-sheet skeleton.
2. Generate candidates and search exact spellings with software, app, and brand qualifiers.
3. Show three naming directions in `.impeccable/critique/lowercase-name-sheet.html`, with both themes side by side. Keep every wordmark lowercase, including Micrographics, as explicitly requested.
4. Ask for optional favorites during the session, refine the recommendation if a response arrives, and preserve the findings here.

## Decisions & trade-offs
Web screening reports observed uses and lack of clear matches in returned results. It does not establish that a name is unused worldwide. Domain registration, trademark databases, phonetic similarity, and exhaustive store searches remain unchecked.

## Candidate shortlist

The following are creative recommendations, not owner selections. No clear named software product appeared for these exact spellings in the returned results on 2026-09-06; the specific non-product uses below remain relevant to further screening.

| Name | Proposed reading / character | Screening notes |
| --- | --- | --- |
| **fucei** | Portuguese: “I poked around.” Curious, casually useful; recommended for personality. | A common verb, so ordinary reviews create substantial search noise. |
| zoiei | Colloquial “I had a look”; close to the requested bizu-like sound. | Dictionary forms and unrelated text surfaced. |
| olhico | A small lookout, with a slightly odd but real Portuguese root. | [Caldas Aulete](https://www.aulete.com.br/olhico) records a regional word for a person keeping a lookout. |
| cadisso | A proposed contraction of “cadê isso?” | [A locality in Angola](https://mapcarta.com/pt/27861388) already has this name. |
| **nemsai** | “I did not even leave.” Recommended for the overlay promise and dry humor. | An [itch.io user](https://itch.io/c/2690645/nemsais-collection) already uses Nemsai. |
| eraali | “It was right there.” A small, deadpan discovery. | Returned matches were largely irrelevant OCR text. |
| liisso | “I read that.” A terse source department. | No clear product match; spelling the double i is a usability cost. |
| **vuzoi** | A proposed invented sound blend around viu / zoio. Recommended for an abstract brand direction. | The string occurs as a made-up experimental stimulus in a [cognition paper](https://www.louisegoupil.co.uk/uploads/1/3/4/8/134803844/goupil___aucouturier_cognition.pdf); not a claim of worldwide novelty. |
| tabnope | Alt-tab + nope. Functional English dry humor. | No clear product match in returned results. |
| loreh | Proposed lore + eh blend, with deliberately modest enthusiasm. | An existing [Minecraft-community handle](https://www.spigotmc.org/members/loreh.2385289/) surfaced. Pronunciation is ambiguous. |
| huhdex | Proposed huh + index blend. A reference book for confusion. | No clear product match; less natural to pronounce in Portuguese. |

## Removed during screening

These are agent screening decisions, not explicit owner declines.

| Name | Observed use / reason |
| --- | --- |
| bizu | Multiple digital products, including [business pages](https://appbizu.com.br/). Kept as a creative reference only. |
| bizoi | An [existing iPhone app](https://apps.apple.com/in/app/bizoi/id6760694022). |
| hmmkay | An [existing software package](https://www.piwheels.org/project/hmmkay). |
| taonde | A named software download in the [Dosvox companion catalog](https://www.acessibilidadeemfoco.com/downloads/dosvox.html). |
| tavaali | Existing publishing usage in [artist publication credits](https://www.vahidvalikhani.com/about). |
| quasei | Existing microphone branding in a [retail listing](https://www.mercadolibre.cl/microfono-vocal-dinamico/up/MLCU61246547). |
| zoviu | The [exact .com is offered for resale](https://www.atom.com/name/Zoviu). This is a domain complication, not proof of an operating software product. |

## Search method and artifact

- Searched exact quoted names, followed by app / software / brand or Portuguese marca qualifiers. Reviewed plain-name results for unusual strings and unrelated meanings.
- Followed up fucei, nemsai, and vuzoi with indexed-domain searches covering GitHub, Product Hunt, the Apple App Store, and Steam. Fucei returned an ordinary app-review use; nemsai and vuzoi returned no results. This is an indexed-web check, not a direct exhaustive search of each catalog.
- The self-contained proof sheet is `.impeccable/critique/lowercase-name-sheet.html` (local and gitignored), derived from the proof-sheet skeleton with current `src/styles.css` tokens. It has eleven names, three directions, both themes side by side, and local shortlist buttons.
- Lowercase is deliberately retained in Micrographics, overriding only the wordmark casing for this experiment. The application is unchanged.

## Status log
- 2026-09-06 — Created the exploration plan. The owner requested lowercase, simple names and dry humor; no candidate has been selected or explicitly declined yet.
- 2026-09-06 — Published `lowercase-name-sheet.html` locally and asked for optional favorite names and explicit declines during the session. Agent recommendations: fucei for personality, nemsai for the product promise, vuzoi for an abstract sound. No owner pick is assumed.
- 2026-09-06 — Completed the requested shortlist and research round. No owner selections or explicit declines were received before delivery; the three recommendations remain exploratory. Browser inspection confirmed the specimens in both themes and a separate preview verified the shortlist interaction. All six `/check` steps passed; vault lint has zero errors and four pre-existing warnings. No application rename is part of this work.
