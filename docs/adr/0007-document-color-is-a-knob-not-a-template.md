---
status: accepted
---

# Document color is a knob, not a template

## Context

The export system shipped nine résumé templates, two of which — `ATS Classic`
and `Modern` — were the same single-column Calibri/all-caps/ruled layout differing
essentially by palette (`Modern` was `Classic` in navy). That is "same template,
different color" redundancy: a new palette does not justify a new `TemplateId`, a
new `.typ` source, a new preview asset, and a new count-pin across the Rust +
frontend + shared-contract guard tests.

At the same time three orthogonal concerns had no home in the model:

1. **Color as a per-export choice.** A dormant accent-override seam already existed
   end-to-end (`RenderOpts.accent`, honored by every `.typ`) but was unwired — there
   was no way for a user to recolor a template without forking it.
2. **Cover-letter design.** A single `letter.typ` served every résumé template;
   there was no way to offer more than one letter arrangement, and no vocabulary to
   describe one without conflating it with the résumé template.
3. **Honest ATS labeling.** No metadata distinguished a parser-safe single-column
   layout from a photo / multi-column one. The ATS-mode toggle keyed on
   `isTwoColumnTemplate`, so the photo-bearing single-column `Lebenslauf` never
   surfaced the toggle and always exported its photo — a silent ATS hazard.

This ADR records the decision behind the template-overhaul series (PRs #590–#594):
**color is a knob, not a template**, plus the two adjacent orthogonality/labeling
decisions the same series settled. It is distinct from [ADR 0004](0004-single-source-user-customizable-accent-color.md),
which owns the **app-UI** accent color (the interactive-element tint driven by
`ThemePrefs`). The export accent here is a **separate concept** — see the
[glossary](../CONTEXT.md) entries for _Document accent_, _Letter layout_, and
_Template tier_.

## Decision

### (a) Remove `Modern`; color becomes a per-export knob

`TemplateId::Modern` and `Template::modern()` are **removed**. `Classic` renders
through the parametric `single_column.typ` (the hardcoded `classic.typ` twin is
deleted; small visual drift was accepted and gated by a golden-diff review). A
saved or stale `"modern"` id **deserializes to `Classic` forever** via the custom
`Deserialize` impl (`export/types.rs`) — it is not an error, just a graceful
fallback, exactly like any other unknown id.

The palette a user wanted from `Modern` is now expressible on **any** template via
the **Document accent** — a per-export hex override next to `templateId` in the
wizard/generation state. It is **not persisted**, **never reads `ThemePrefs`**, and
`None` (the default) leaves the template's built-in palette untouched (zero output
change until opted in). The same hex is threaded through one validator on every
render path (see consequence "Accent validation is single-source").

### (b) Letter layout is orthogonal to the résumé template

Cover-letter design is modeled as `LetterLayout { Classic, Refined, Banded }`
(contract field `letterLayoutId`, graceful-fallback `Deserialize`). A **layout owns
only arrangement/composition**. The **palette and fonts always inherit** from the
selected résumé `TemplateId` via `style_from_template` (which produces a
`LetterStyle`), so a letter keeps matching its résumé family regardless of layout.
**Market conventions own the semantics** (date position, subject line, recipient
block) via `LetterMarketConventions`; where a convention and a layout's arrangement
conflict, the convention wins (e.g. DE DIN date-top-right). The governing rule:
**new letter layouts gate structural elements on `data.opts` conventions, never on
the layout id** — composition and semantics stay separated.

### (c) Template tier is honest ATS labeling

Templates carry a `TemplateTier { Ats, Design }` metadata field (Rust `Template` +
frontend registry). It is **metadata only, no render behavior**: it drives the
grouped gallery (ATS-Safe / Design sections + badge) and, critically, **which
templates surface the ATS-mode toggle**. The gate moves from `isTwoColumnTemplate`
to `isDesignTier`, so the photo-bearing single-column `Lebenslauf` (design tier)
now surfaces the toggle and drops its photo in ATS mode. The split is honest about
a real cost: single-column layouts parse at roughly 96–97% accuracy in common ATS
parsers, versus roughly 61% for two-column layouts — so "design" templates
truthfully advertise that they collapse to a linear single column under ATS mode.

## Considered options

1. **Color = per-export knob; keep `Modern` as a second template.** Rejected: keeps
   the "same template, different color" redundancy the knob was meant to erase, and
   forces every palette idea to be a new `TemplateId` + `.typ` + preview + count-pin.
2. **Persist the Document accent in `ThemePrefs` (reuse ADR 0004's storage).**
   Rejected: conflates the app-UI accent (one durable user preference) with a
   per-document styling choice (many, ephemeral, per export). The export backend
   must never read `ThemePrefs` — a résumé's color is a property of that export, not
   of the app's chrome.
3. **A `LetterTemplate` parallel to `TemplateId` (letters pick their own palette).**
   Rejected: doubles the palette surface and lets a letter visually diverge from its
   résumé. Inheritance (layout = arrangement, palette = résumé template) keeps the
   pair coherent with one palette source.
4. **Gate the ATS toggle on `isTwoColumnTemplate` (no tier metadata).** Rejected: it
   was the existing behavior and it silently mis-classified single-column-with-photo
   `Lebenslauf` as ATS-safe. Tier is the honest predicate; two-column-ness is an
   implementation detail of _some_ design-tier templates.
5. **Three ADRs (accent, letter orthogonality, tier).** Rejected: they share one
   thesis — a visual dimension (color / letter arrangement / ATS honesty) is a knob
   or a label on the existing template, not a reason to multiply templates. One ADR
   keeps the rationale together.

## Consequences

- **Saved `"modern"` ids deserialize to `Classic` forever.** The fallback arm is a
  permanent part of the contract; removing it would break any stored generation that
  still names `"modern"`. A pinning test asserts `"modern" → Classic`.
- **Accent validation is single-source.** The résumé-PDF path validates the hex via
  `typst_engine::normalise_accent`; the cover-letter and DOCX paths recolor via
  `Template::with_accent_override`, whose `parse_accent_rgb` **delegates to the same
  `normalise_accent`**. One validator, so PDF and DOCX can never disagree on whether
  an accent is valid.
- **The recolored surface legitimately differs per template family and per format.**
  The accent overrides each template's accent _role_: on single-column templates the
  accent is chiefly the link color; on premium templates it's headings/bands. Across
  formats, the DOCX backend applies the override to emphasis runs
  (`emphasis_color`). So the same hex visibly lands on different surfaces depending
  on template and format — this is intended, not a bug. (DOCX link-alignment with the
  accent is a follow-up candidate.)
- **Letter layouts can't diverge from their résumé palette.** Because palette/fonts
  come from `style_from_template`, a new layout only ever adds an arrangement; it
  cannot introduce a new color story or break a market convention (those stay in
  `data.opts`). DOCX approximates the PDF layouts: `Banded`'s angled polygon becomes
  flat accent-tinted paragraph shading, and PDF small-caps become uppercase.
- **Tier is advisory, not enforced.** `TemplateTier` changes no rendering by itself;
  it only groups the gallery and picks the ATS-toggle set. A design-tier template
  still produces a valid document with ATS mode off — the label just tells the user
  (truthfully) that its two-column / photo form is the riskier ATS choice.
- **The template roster grew to twelve without multiplying color variants.** The
  four new templates (Cadence, Regent, Aria, Saffron) each earn a `TemplateId` by
  distinct layout/typography, not palette — the redundancy this ADR removed does not
  return.

## References

- Template IDs + `"modern" → Classic` fallback: `apps/desktop/src-tauri/src/export/types.rs` (`TemplateId`, `LetterLayout`).
- Template registry + tier + accent override: `apps/desktop/src-tauri/src/export/templates/mod.rs` (`Template`, `TemplateTier`, `with_accent_override`, `parse_accent_rgb`).
- Accent validator (single source): `apps/desktop/src-tauri/src/export/typst_engine/` (`normalise_accent`).
- Section placement / two-column / tier gates: `apps/desktop/src-tauri/src/theme/mod.rs` (`placement_for`, `is_two_column`).
- Letter layout dispatch + inherited style: `apps/desktop/src-tauri/src/export/typst_engine/engine.rs` (`letter_source`), `typst_engine/letter.rs` (`LetterStyle`, `style_from_template`), market conventions `src/locale/letter.rs`.
- DOCX letter approximation: `apps/desktop/src-tauri/src/export/docx/mod.rs` (`generate_cover_letter_docx`).
- Frontend registry + tier gate: `apps/desktop/src/renderer/lib/generate/templates/templates.ts` (`TemplateId`, `tier`, `isDesignTier`).
- Shared contract: `packages/shared/src/ipc/contracts/documents.ts` (`TemplateId`, `accent`, `letterLayoutId`).
- App-UI accent (distinct concept): [ADR 0004](0004-single-source-user-customizable-accent-color.md).
- Full contract + roster: [`docs/EXPORT_TEMPLATES.md`](../EXPORT_TEMPLATES.md); glossary: [`docs/CONTEXT.md`](../CONTEXT.md).
