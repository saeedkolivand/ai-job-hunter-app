/**
 * Shared form-field signal primitives — label text, visibility, and the
 * ambiguous/sensitive denylist. Factored out of `autofill.ts` (PR 5 of the
 * extension roadmap) so BOTH `fill.ts` (assisted autofill, via
 * `autofill.ts`) and `capture.ts` (answers capture, via
 * `answers-capture.ts`) share ONE definition of "what counts as a labelled /
 * visible / ambiguous field" — never two copies that could drift.
 *
 * Pure DOM — no extension APIs, no network — so it is unit-testable against a
 * jsdom document. Behavior is UNCHANGED from what previously lived inline in
 * `autofill.ts`; this is a pure extraction (its existing tests pin the
 * behavior and pass unmodified).
 *
 * Build note: `fill.js` and `capture.js` are each injected via
 * `chrome.scripting.executeScript({ files: [...] })` as CLASSIC scripts (no
 * ES module support) — they must bundle with ZERO `import` statements. Since
 * BOTH now genuinely share this module at runtime, `vite.config.ts` builds
 * each in its OWN isolated Rollup pass (the `injectedEntries` plugin) so this
 * file is inlined into EACH bundle rather than hoisted into a shared chunk
 * that either classic script would then have to `import`.
 */

/**
 * Substrings that make a field ambiguous or sensitive — a match on the label /
 * name / id / placeholder skips the field entirely (under-fill over mis-fill
 * for autofill; skip-don't-capture for answers capture). Includes the grilled
 * set (referrer/emergency/confirm/manager/parent) plus the fields most likely
 * to receive the WRONG identity on a job-application form
 * (company/employer/recruiter), login/search noise, and — defense-in-depth —
 * sensitive PII categories that should never be touched even by accident
 * (SSN/tax id, passport, date of birth, national id, driver's license,
 * bank/IBAN, visa status).
 */
const AMBIGUOUS = [
  'emergency',
  'confirm',
  'manager',
  'supervisor',
  'parent',
  'guardian',
  'company',
  'employer',
  'organization',
  'organisation',
  'recruiter',
  'username',
  'user name',
  'password',
  'captcha',
  'coupon',
  'promo',
  'maiden',
  'ssn',
  'social security',
  'tax',
  'passport',
  'dob',
  'birth',
  'date of birth',
  'national id',
  'national insurance',
  'license',
  'licence',
  'iban',
  'bank account',
  'routing number',
  'sort code',
  'visa status',
  'green card',
  'immigration status',
  // ── Localized third-party / reference / emergency-contact fields ──────────
  // The EU-language equivalents of the referrer/reference/emergency/supervisor/
  // contact-person categories already protected in English above. Without
  // these, a "Name des Ansprechpartners" / "Name der Referenzperson" field
  // matches the generic `\bname\b` catch-all in `matchNamedKey` and mis-fills
  // with the applicant's OWN name. All are long, distinctive substrings with no
  // English-word collision (the short/collision-prone ones live in
  // {@link AMBIGUOUS_WORDS} / {@link AMBIGUOUS_PREFIXED} below).
  'ansprechpartner', // DE contact person
  'notfall', // DE emergency (covers notfallkontakt/notfallnummer)
  'referenz', // DE reference (covers referenzperson/referenznummer)
  'vorgesetzt', // DE supervisor (vorgesetzte/vorgesetzter)
  'personne a contacter', // FR person to contact
  'recruteur', // FR recruiter
  'contacto de emergencia', // ES emergency contact
  'persona de contacto', // ES contact person
  'referencia', // ES reference
  'emergencia', // ES emergency
  'reclutador', // ES recruiter
  'contatto di emergenza', // IT emergency contact
  'referente', // IT reference/contact person
  'emergenza', // IT emergency
  'contactpersoon', // NL contact person
  'noodcontact', // NL emergency contact
  'osoba kontaktowa', // PL contact person
  'kontakt alarmowy', // PL emergency contact
  'referencje', // PL references
  // ── Localized sensitive PII (date of birth / national-id / tax / social) ──
  // Long, distinctive substrings (accent-free — the signal is diacritic-folded
  // by `textSignal`). A false match here only ever SKIPS a field (never
  // mis-fills / mis-captures), so this is the safe direction.
  'geburtsdatum', // DE date of birth
  'geburtstag', // DE birthday
  'steuernummer', // DE tax number
  'sozialversicherung', // DE social security
  'ausweis', // DE id card
  'date de naissance', // FR date of birth
  'numero de securite sociale', // FR social-security number
  'fecha de nacimiento', // ES date of birth
  'codice fiscale', // IT tax code
  'data di nascita', // IT date of birth
  'geboortedatum', // NL date of birth
  'data urodzenia', // PL date of birth
  'pesel', // PL national id
  'data de nascimento', // PT date of birth
  'personnummer', // SV/NO national id
  'cpr-nummer', // DA national id
  'fodselsnummer', // NO national id (ø folded to o by `stripDiacritics`)
  'henkilotunnus', // FI national id
];

/**
 * Ambiguous/sensitive terms that are too SHORT or too English-collision-prone
 * to match as plain substrings — matched on `\b` WORD boundaries instead (the
 * signal is accent-free and lowercased, so keywords are too). A bare substring
 * would wrongly SKIP a legitimate field: `dni` ⊂ Polish "poprzedni", `bsn` is a
 * 3-letter fragment, `urgence` ⊂ "insurgence". `\burgence\b` still catches the
 * FR "contact d'urgence" form (the apostrophe is a word boundary).
 */
const AMBIGUOUS_WORDS = /\b(?:dni|bsn|urgence)\b/;

/**
 * Denylist terms that must not match INSIDE a longer word, anchored on their
 * LEADING side only.
 *
 * As plain substrings these silently skipped extremely common fields:
 * `referr` ⊂ "**preferr**ed" (→ "Preferred first name", "Preferred pronouns"),
 * `search` ⊂ "re**search**" (→ "Research experience"), and `reference` ⊂
 * "p**reference**s" (→ "Work preferences"). Skipping is doubly costly here: such
 * a field is neither filled by autofill NOR captured by answers-capture, so the
 * answer is lost rather than merely deferred.
 *
 * Only the leading side is anchored, for the same two reasons as
 * {@link NAMED_KEY_PATTERNS}: `_` is a regex word character, so `\b` would stop
 * matching `job_search` / `reference_name`, and the trailing side must stay open
 * for `referral`, `referrer`, `references` and `searchTerm`.
 */
const AMBIGUOUS_PREFIXED = /(?:^|[^a-z])(?:referr|reference|search)/;

/**
 * Name fields that belong to SOMEONE ELSE (a reference, a referrer, a spouse /
 * child / dependent / beneficiary) or to a name we hold no value for (a former /
 * previous / other / middle / additional name, a Japanese kana reading) — all
 * must be SKIPPED.
 *
 * Why a dedicated rule rather than more {@link AMBIGUOUS} entries: that list is
 * PROSE-shaped (plain substrings of a rendered label) and
 * {@link AMBIGUOUS_PREFIXED} is LEADING-anchored, so both miss the camelCase
 * ATTRIBUTE spellings that {@link NAMED_KEY_PATTERNS} now matches —
 * `jobReferenceFirstName`, `myReferrerFirstName`, `spouseFirstName`,
 * `dependentLastName`, `previousLastName`. Widening the name patterns without
 * this rule would write the USER's name into a third party's box: a mis-fill,
 * which this module's contract forbids.
 *
 * Shape: a relation/qualifier stem, then (after any run of separators, and an
 * optional first/last/full/middle/given/family qualifier) a REQUIRED trailing
 * `name`/`nm` token. Requiring that trailing token is what keeps
 * "work preferences" / "research experience" fillable — they have a stem but no
 * name token. The trailing side stays open (`referenceFirstName1`). The
 * kana/furigana reading is listed BOTH as a stem (`kana_last_name`) and as a
 * trailing alternative, because that qualifier can follow the name token
 * (`lastNameKana`).
 *
 * The reference/referrer family lives in its own {@link AMBIGUOUS_REFERENCE_NAME}
 * because it — and only it — needs the "preferred" exemption; keeping it out of
 * this rule means an unrelated "prefer" elsewhere in the signal can never
 * un-deny a spouse/middle/kana field.
 */
const AMBIGUOUS_NAME_COMPOUND =
  /(?:spouse|child|dependen|beneficiar|former|previous|other|aka|also[\s_-]*known[\s_-]*as|middle|additional|kana|furigana)[a-z]*[\s_-]*(?:first|last|full|middle|given|family)?[\s_-]*n(?:ame|m)|n(?:ame|m)[\s_-]*(?:kana|furigana)|furigana/;

/**
 * {@link AMBIGUOUS_NAME_COMPOUND}'s shape for the reference/referrer family
 * (`jobReferenceFirstName`, `myReferrerLastName`, `refereeFirstName`).
 *
 * Deliberately UNANCHORED, unlike {@link AMBIGUOUS_PREFIXED}: a `[^p]` character
 * guard (the obvious way to spare "**p**referred"/"**p**reference") exempts every
 * p-terminated prefix, and `empReferenceFirstName`,
 * `groupReferenceFirstName`, `backupReferenceFirstName`, `topReferenceFirstName`
 * … are ordinary HRIS spellings that were each filled with the applicant's own
 * name. The exemption is done by WORD in {@link isAmbiguousSignal} instead.
 */
const AMBIGUOUS_REFERENCE_NAME =
  /(?:referr|refere)[a-z]*[\s_-]*(?:first|last|full|middle|given|family)?[\s_-]*n(?:ame|m)/;

/**
 * True when a field's {@link textSignal} is ambiguous or sensitive and must be
 * SKIPPED by both autofill ({@link textSignal} → `isCandidateField`) and
 * answers-capture (`isCapturable`). Combines the plain-substring {@link AMBIGUOUS}
 * denylist with the word-anchored {@link AMBIGUOUS_WORDS}, the
 * leading-anchored {@link AMBIGUOUS_PREFIXED} and the compound
 * {@link AMBIGUOUS_NAME_COMPOUND} ones, so the two consumers can never disagree
 * on what counts as ambiguous.
 *
 * {@link AMBIGUOUS_REFERENCE_NAME} is the one rule with an exemption: a signal
 * where "prefer" STARTS a word is the ubiquitous "Preferred first name" /
 * `preferred_first_name` field (its `refer` exists only because "prefer"
 * contains one) and must keep filling. The anchor is what separates it from
 * `empReferenceFirstName` / `topReferenceFirstName`, where the same letters are
 * a prefix + "reference". A camelCase `candidatePreferredFirstName` is
 * collateral of that ambiguity — the two are spelled identically apart from the
 * word boundary, so it is skipped: an under-fill, which this module prefers to
 * writing the user's name into a reference's box.
 */
export function isAmbiguousSignal(signal: string): boolean {
  return (
    AMBIGUOUS.some((w) => signal.includes(w)) ||
    AMBIGUOUS_WORDS.test(signal) ||
    AMBIGUOUS_PREFIXED.test(signal) ||
    AMBIGUOUS_NAME_COMPOUND.test(signal) ||
    (AMBIGUOUS_REFERENCE_NAME.test(signal) && !/(?:^|[^a-z])prefer/.test(signal))
  );
}

/**
 * True when `el` or ANY ancestor is hidden — via the `hidden` attribute or
 * COMPUTED style (not just inline `style`): `display:none`/`visibility:hidden`,
 * `opacity:0`, off-screen absolute/fixed positioning (`left`/`top` shoved past
 * -9999px — the classic honeypot trap), or a box whose computed `width` AND
 * `height` are BOTH exactly `0px`. Computed style (not just inline `style`) is
 * what catches an external-stylesheet / `<style>` CSS-class honeypot — this is
 * how anti-bot honeypot fields are commonly planted on real ATS forms
 * (Greenhouse/Lever/Workday). An inline-only or display/visibility-only check
 * would fill/capture them, and a filled invisible field is worse than an
 * ordinary mis-fill (the user can't see it to undo, and it can flag them as a
 * bot).
 *
 * NOT caught, deliberately: clip-based hiding (`clip:rect(0,0,0,0)`/
 * `clip-path`) or a single-dimension-zero box (e.g. the `width:1px;height:1px`
 * shape common to `.sr-only`-style utility classes) — that is also exactly how a
 * LEGITIMATE screen-reader-only field is hidden visually while staying
 * functionally real, so treating it as hidden (and skipping it) would be
 * wrong. Only an unambiguous honeypot shape — display/visibility/opacity-off,
 * off-screen, or BOTH dimensions zero — is treated as hidden.
 *
 * Deliberately `getComputedStyle`-ONLY — never `getBoundingClientRect`/
 * `offsetWidth`/layout reads. jsdom (the test environment) always reports those
 * as zero, which would make every field — including normal visible ones — read
 * as hidden. Computed style has no such gap: a real field's computed `width` is
 * `auto`/a real length (never the literal string `'0px'`), its `position` is
 * `static`, and its `opacity` is `1`, so this stays jsdom-safe.
 */
export function isHidden(el: HTMLElement): boolean {
  const view = el.ownerDocument.defaultView;
  let node: HTMLElement | null = el;
  while (node) {
    if (node.hidden) return true;
    const cs = view?.getComputedStyle(node);
    if (cs) {
      if (cs.display === 'none' || cs.visibility === 'hidden') return true;
      if (Number.parseFloat(cs.opacity) === 0) return true;
      if (
        (cs.position === 'absolute' || cs.position === 'fixed') &&
        (Number.parseFloat(cs.left) <= -9999 || Number.parseFloat(cs.top) <= -9999)
      )
        return true;
      if (cs.width === '0px' && cs.height === '0px') return true;
    }
    node = node.parentElement;
  }
  return false;
}

/** Per-source character cap for {@link labelText}. A label is a short phrase;
 *  an `aria-labelledby` id may point at a whole CONTAINER (a fieldset, a card),
 *  whose `textContent` is unbounded — and this string is persisted as the
 *  answers-capture question key and sent over the bridge, so it must not grow
 *  without limit. 300 is far above any real label and well below "swallowed the
 *  page". */
const LABEL_SOURCE_MAX = 300;

/** CSS.escape when available (jsdom + browsers), else a conservative fallback. */
function escapeId(id: string): string {
  if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') return CSS.escape(id);
  return id.replace(/["\\]/g, '\\$&');
}

/** The associated label text for a form element: `<label for>` + any wrapping
 *  `<label>` + every element referenced by `aria-labelledby`. Takes
 *  `HTMLElement` (not just `HTMLInputElement`) so it works identically for
 *  `<textarea>`/`<select>` — every member it touches (`id`/`closest`/
 *  `getAttribute`) is generic to `Element`, not input-specific.
 *
 *  `aria-labelledby` is the ONLY label many modern ATS forms expose: Workday and
 *  Ashby render their field labels as sibling `<div>`/`<span>`s wired by id
 *  rather than a `<label for>`, so without this resolution those fields carried
 *  an empty label and were matched from `name`/`id`/`placeholder` alone (or, for
 *  answers-capture, skipped as unlabelled). The attribute is an id LIST
 *  (space-separated, in reference order) — each referenced element's
 *  `textContent` is appended, same shape as the label text above. `getElementById`
 *  needs no escaping (unlike the `label[for=…]` selector) and is jsdom-safe; no
 *  layout/computed-style read is involved.
 *
 *  Each source element is counted ONCE (`seen`): the common React-Aria /
 *  headless-UI shape points `aria-labelledby` at the very `<label for>` that is
 *  already picked up above (and a wrapping `<label for>` matches twice on its
 *  own), which would otherwise yield "First name First name" — harmless for
 *  keyword matching, but `answers-capture.ts` persists this string as the
 *  QUESTION key, so a duplicated label is a duplicated stored question. */
export function labelText(el: HTMLElement): string {
  const doc = el.ownerDocument;
  const seen = new Set<Element>();
  let text = '';
  const append = (node: Element | null): void => {
    if (!node || seen.has(node) || !node.textContent) return;
    seen.add(node);
    text += ` ${node.textContent.slice(0, LABEL_SOURCE_MAX)}`;
  };

  if (el.id) append(doc.querySelector(`label[for="${escapeId(el.id)}"]`));
  append(el.closest('label'));
  const labelledBy = el.getAttribute('aria-labelledby');
  if (labelledBy) {
    for (const id of labelledBy.split(/\s+/)) {
      if (id) append(doc.getElementById(id));
    }
  }
  // Collapse the source markup's newlines/indentation into single spaces: this
  // string is not only matched against, it is PERSISTED as the answers-capture
  // question key and sent over the bridge, so "Why\n    this role?" and
  // "Why this role?" must be one question, not two.
  return text.replace(/\s+/g, ' ');
}

/** Fold an accented EU label to ASCII so it matches the accent-free keyword
 *  table in {@link matchNamedKey} and the {@link AMBIGUOUS} denylist. Two steps,
 *  because NFD alone is not enough: (1) NFD-decompose then strip combining marks
 *  (é → e, ä → a, ż → z, å → a); (2) an explicit fold for the atomic letters
 *  that have NO NFD decomposition (ø, æ, ł, đ, ß) — without step 2, "Fødselsnummer"
 *  or "Szkoła" would keep their ø/ł and never match "fodselsnummer"/"szkola".
 *  Kept LOCAL to this file (no import — this module is inlined verbatim into the
 *  classic-injected `fill.js`/`capture.js` bundles, which forbid `import`);
 *  `autofill.ts` keeps its own `normalizeLabel` for the extra-link matcher. */
function stripDiacritics(s: string): string {
  return s
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .replace(/[øØ]/g, 'o')
    .replace(/[æÆ]/g, 'ae')
    .replace(/[łŁ]/g, 'l')
    .replace(/[đĐ]/g, 'd')
    .replace(/[ßẞ]/g, 'ss');
}

/** The accent-free, lowercased free-text signal (name/id/placeholder/aria-label/
 *  label) used both for autofill's Tier-2 field matching and the answers-capture
 *  denylist check. Diacritics are stripped here (not per keyword) so BOTH the
 *  named-key table and the AMBIGUOUS denylist can be written accent-free and a
 *  German/French/Polish/… label still matches. Takes `HTMLElement` for the same
 *  reason as `labelText`. */
export function textSignal(el: HTMLElement): string {
  return stripDiacritics(
    [
      el.getAttribute('name') ?? '',
      el.id,
      el.getAttribute('placeholder') ?? '',
      el.getAttribute('aria-label') ?? '',
      labelText(el),
    ].join(' ')
  ).toLowerCase();
}

/** The same normalization as {@link textSignal}, but from the field's OWN
 *  ATTRIBUTES only (`name`/`id`/`autocomplete`) — no placeholder, aria-label or
 *  label prose.
 *
 *  {@link matchNamedKey} takes this as a second, narrower signal because the two
 *  carry different EVIDENCE. Prose is written for a human and freely names both
 *  halves of a name ("First and Last Name" is a single full-name box; a "Full
 *  Name" group heading sits above separate first/last boxes), while an attribute
 *  names the ONE field it is on. A rule that must not be fooled by a group
 *  heading — the `fullName` row's `denyAttribute` — therefore reads only this. */
export function attributeSignal(el: HTMLElement): string {
  return stripDiacritics(
    [el.getAttribute('name') ?? '', el.id, el.getAttribute('autocomplete') ?? ''].join(' ')
  ).toLowerCase();
}

/** The last (field) token of an `autocomplete` attribute value, e.g.
 *  "shipping email" → "email". `''` for a missing/`off`/`on` attribute. Takes
 *  `HTMLElement` (not just `HTMLInputElement`) so `answers-capture.ts` can call
 *  it on a `<textarea>`/`<select>` too — an `autocomplete` attribute there
 *  simply normalizes to `''`/`off`, same as no match. Shared so autofill's
 *  Tier-1 token reading and answers-capture's identity check never drift on
 *  what the raw token is. */
export function autocompleteToken(el: HTMLElement): string {
  const raw = (el.getAttribute('autocomplete') ?? '').trim().toLowerCase();
  if (!raw || raw === 'off' || raw === 'on') return '';
  return raw.split(/\s+/).at(-1) ?? '';
}

/**
 * Map a standard `autocomplete` {@link autocompleteToken} to autofill's Tier-1
 * logical key, or `null` for a token with no fill/identity meaning here.
 * Mirrors `matchFieldKey`'s Tier-1 switch (`autofill.ts`) — factored out here
 * so `isCapturable` (`answers-capture.ts`) can exclude a field whose
 * `autocomplete` attribute marks it as identity (e.g. `autocomplete="name"`)
 * WITHOUT duplicating the token→key literals in a second copy.
 */
export function matchAutocompleteKey(token: string): string | null {
  switch (token) {
    case 'email':
      return 'email';
    case 'tel':
    case 'tel-national':
    case 'tel-local':
      return 'phone';
    case 'given-name':
      return 'firstName';
    case 'family-name':
      return 'lastName';
    case 'name':
      return 'fullName';
    case 'url':
      return 'website';
    // Only the city-level address token maps to the single free-text location;
    // street/postal/state/country sub-parts can't be filled from one string.
    case 'address-level2':
      return 'location';
    default:
      return null;
  }
}

/**
 * Resolve a lowercased field {@link textSignal} to a known identity key, or
 * `null` when it doesn't unambiguously match one. This is autofill's "Tier 2"
 * signal matching (`matchFieldKey` in `autofill.ts`), factored out so it's
 * shared with answers-capture: `isCapturable` (`answers-capture.ts`) calls it
 * to EXCLUDE any field whose signal identifies it as one of these keys — a
 * filled "Full Name" or "LinkedIn URL" text field must never be captured into
 * `Application.answers` as if it were a genuine application question. Pure
 * string matching — no element/autocomplete-attribute lookup (that stays
 * `autofill.ts`-only "Tier 1", since capture also runs against `<select>`/
 * `<textarea>` which don't carry the same autocomplete semantics).
 */
/**
 * Ordered, first-match-wins keyword table for {@link matchNamedKey}. Each
 * `pattern` runs against the accent-free, lowercased {@link textSignal}, so
 * every keyword is written WITHOUT diacritics (universite, not université) and
 * a `\b` anchor is used for short / collision-prone terms.
 *
 * Widened beyond English to the major EU languages
 * (DE/FR/ES/IT/NL/PL/PT/SV/DA, plus NO/FI where the words coincide). Two
 * ordering choices matter:
 *  - the COMBINED full-name phrases ("nombre completo", "imie i nazwisko", …)
 *    run BEFORE first/last, so such a field resolves to `fullName` rather than
 *    grabbing just its first (or last) token — those single tokens are
 *    substrings of the combined phrase;
 *  - a name term that is also a substring of a "username"/"company"/full-name
 *    phrase carries a negative lookahead (e.g. `nombre(?!… de …)`) so a
 *    "Nombre de usuario" / "Nombre de la empresa" field never mis-fills as a
 *    first name.
 *
 * The generic bare-"Name" catch-all (with its education/company/user denylist)
 * is NOT in this table — it runs last, in {@link matchNamedKey}, only after
 * every specific pattern misses.
 */
/**
 * The owner of a "name" that is NOT the applicant: an account/system name
 * (user/file/nick/screen/display), an organization, or a
 * school/course/degree/major.
 * Localized (accent-free — the signal is diacritic-stripped) so
 * "Name der Schule" / "Nom de l'entreprise" never receive the person's name.
 *
 * `\bfirma\b` is anchored — a bare `firma` substring wrongly matches the English
 * words "affirmative"/"confirmation", which would stop a legitimate
 * "Name (Affirmative Action)" EEO field from filling.
 *
 * Hoisted to a const because it now gates TWO places that must never drift: the
 * bare-"Name" catch-all in {@link matchNamedKey} (where it has always run) and
 * the `fullName` row's {@link NamedKeyPattern.deny} below — without the latter,
 * the attribute spellings the row gained (`school_full_name`,
 * `universityFullName`, `degreeFullName`, …) walked straight past a denylist the
 * prose spelling ("University Name") has always respected.
 */
const NON_PERSON_NAME_OWNER =
  /user|file|nick|screen|display|business|org|school|institution|university|college|degree|course|program|major|certificat|schule|hochschule|universitat|benutzer|\bfirma\b|unternehmen|ecole|universite|entreprise|societe|utilisateur|escuela|universidad|empresa|usuario|scuola|universita|azienda|utente|szkola|uczelnia|uzytkownik|gebruiker|bedrijf|foretag|anvandare|virksomhed|bruger|yritys|kayttaja/;

/**
 * The first/last spellings, as the `fullName` row's
 * {@link NamedKeyPattern.denyAttribute} — matched against the ATTRIBUTE signal
 * ({@link attributeSignal}) only, never the prose one.
 *
 * A GROUP label reaches a field's signal through `aria-labelledby` (and, for a
 * wrapping `<label>`, through {@link labelText}), so a "Full Name" group heading
 * above `first_name` / `last_name` boxes lands the phrase "full name" in BOTH
 * their signals. Since the `fullName` row runs first, each box would then take
 * the WHOLE name. When the field's OWN ATTRIBUTE says first/last, that is the
 * more specific evidence and wins.
 *
 * Reading only the attribute signal is load-bearing, not a refinement: the
 * separator class accepts a space, so against the full signal this also matched
 * PROSE — the single full-name box placeheld "First and Last Name" was vetoed
 * out of `fullName` and fell through to `lastName`, receiving only the surname.
 * (Dropping `\s` from the class is NOT the fix — it re-opens the mirror case, a
 * prose `aria-label="First name"` under a "Full Name" heading.) The DE/ES/IT/PL
 * combined phrases contain none of these tokens either way.
 */
const ATTRIBUTE_FIRST_LAST_NAME =
  /(?:first|given|fore)[\s_-]*(?:name|nm)|(?:^|[^a-z])fname|(?:last|family)[\s_-]*(?:name|nm)|(?:^|[^a-z])lname/;

/** One row of {@link NAMED_KEY_PATTERNS}: the key it resolves to, the signal
 *  pattern that claims it, and optional vetoes — the row-scoped equivalent of
 *  the catch-all's own denylist. A veto makes the matcher fall through to the
 *  rows below, and finally to the catch-all. */
interface NamedKeyPattern {
  key: string;
  pattern: RegExp;
  /** Vetoes the row when it matches the FULL signal (attributes + prose). */
  deny?: RegExp;
  /** Vetoes the row only when it matches the field's own ATTRIBUTE signal
   *  ({@link attributeSignal}) — for evidence that a human-facing label may
   *  legitimately carry but an attribute may not. */
  denyAttribute?: RegExp;
}

const NAMED_KEY_PATTERNS: readonly NamedKeyPattern[] = [
  { key: 'linkedin', pattern: /linkedin/ },
  { key: 'github', pattern: /github/ },
  { key: 'website', pattern: /portfolio|personal (web ?site|site)/ },
  // `email`/`e-mail` already cover most EU forms (e-mail-adresse, adresse
  // e-mail, indirizzo e-mail, …); only the non-"mail" spellings are added.
  { key: 'email', pattern: /email|e-mail|\bcorreo\b|\bcourriel\b|sahkoposti/ },
  // `telefon` (substring) covers telefon(nummer)/telefono/telefone across
  // DE/ES/IT/PT/SV/DA/NO/PL; `telefoon` (NL) and `telephone` (FR/EN) differ.
  // `handy`/`mobil` stay `\b`-anchored (bare `handy` ⊂ "handyman", bare `mobil`
  // ⊂ "automobil…"), so the concatenated DE compounds are listed explicitly.
  //
  // `phone`/`mobile` are anchored on their LEADING side for the same reason the
  // authors anchored `handy`/`mobil`: bare `phone` ⊂ "smartphone"/"iphone"/
  // "microphone" and bare `mobile` ⊂ "automobile", so a "Smartphone model" field
  // received the user's phone number — a mis-fill, which this module's contract
  // forbids ("under-fills rather than mis-fills"). The known compound prefixes
  // (`cell`/`home`/`work`/`day`/`tele`phone) are ENUMERATED so they keep matching
  // — camelCase compounds flatten to the same lowercased signal (`workPhone` →
  // `workphone` → `work`+`phone`), while a non-enumerated `headphone`/`smartphone`
  // deliberately does NOT match. The TRAILING side is deliberately left open — `_` is a word
  // character, so a `\b`/trailing anchor would stop matching the very common
  // `phone_number`, `phoneNumber` and `mobile_number` field names. `(?:^|[^a-z])`
  // is used rather than a lookbehind because these patterns are only ever
  // `.test()`ed, so consuming the boundary character is harmless.
  {
    key: 'phone',
    pattern:
      /(?:^|[^a-z])(?:cell|home|work|day|tele)?phone|(?:^|[^a-z])mobile|telefon|telefoon|handynummer|handytelefon|mobilnummer|mobiltelefon|\bhandy\b|\bmobil\b|puhelin/,
  },
  // Combined full-name phrases — MUST precede first/last (see table doc): both
  // the "completo"-style forms AND the "first AND last" conjunction forms
  // ("Vor- und Nachname", "Nombre y apellidos", "Nome e cognome"), which would
  // otherwise PARTIALLY fill via the first/last patterns below (nachname →
  // lastName, nombre → firstName). `-?` tolerates the elided-hyphen DE/NL forms.
  //
  // `full[\s_-]*name` (was the space-only `\bfull name\b`) so the ATTRIBUTE
  // spellings resolve here too — `fullname`, `full_name`, `full-name`, and the
  // camelCase `fullName` (which flattens to `fullname` in the lowercased
  // signal). Left unanchored on both sides, unlike `phone`/`city`: no English or
  // EU word CONTAINS "fullname", so there is no collision to guard against,
  // while an anchor would break the very common `candidateFullName`-style
  // compound.
  {
    key: 'fullName',
    // The English conjunction forms ("First and Last Name", "First name & last
    // name", "first/last name") join the localized ones: a SINGLE box asking for
    // both halves is a full-name field, and without them it fell to the
    // `lastName` row below (its `last name` matches) and received only the
    // surname.
    pattern:
      /full[\s_-]*name|first[\s_-]*(?:name)?[\s_-]*(?:and|&|\+|\/)[\s_-]*last[\s_-]*name|vollstandiger name|vor-? und nachname|nom complet|prenom et nom|nombre completo|nombre y apellidos?|nome completo|nome e cognome|imie i nazwisko|volledige naam|voor-? en achternaam|fullstandigt namn/,
    // Veto 1 (whole signal): a non-person owner — the same denylist the
    // bare-"Name" catch-all applies, so "University Full Name" is refused in
    // prose AND attribute spellings.
    deny: NON_PERSON_NAME_OWNER,
    // Veto 2 (ATTRIBUTE signal only): the field's own name/id says first/last,
    // so a "Full Name" GROUP heading in its prose must not win. See the const.
    denyAttribute: ATTRIBUTE_FIRST_LAST_NAME,
  },
  // First/last name — the separator between the two words is OPTIONAL
  // (`[\s_-]*`), because a form field's strongest signal is usually its
  // `name`/`id` ATTRIBUTE, not a prose label: `first_name`, `firstName`
  // (→ `firstname` once lowercased), `given-name` and Greenhouse's
  // `job_application[first_name]` are all the same field as a "First name"
  // label. Before this, the space-only `first name`/`last name` patterns missed
  // every one of them, and the HYPHENATED spellings were actively harmful: `-`
  // is a non-word character, so `first-name`/`family-name` fell through to the
  // generic `\bname\b` catch-all in `matchNamedKey` and received the FULL name
  // in BOTH boxes.
  //
  // Anchoring follows the `phone`/`city` rule — anchor only what collides.
  // `first`/`given`/`fore`/`last`/`family` + `name` collide with nothing, so
  // they stay open on both sides (`applicantFirstName`, `first_name_1` and
  // `firstNameInput` must all keep matching). The bare abbreviations DO collide
  // and are LEADING-anchored: `lname` ⊂ "fullname" (which must stay `fullName`,
  // not `lastName`) and `fname` is short enough to hide inside a future
  // compound. The trailing side stays open for `fname_1`/`lnameInput`.
  // `nm` is Taleo's abbreviation (`firstNm`/`lastNm`).
  //
  // `nombre` (ES) and `nome` (IT/PT) mean "name" — excluded when they head a
  // username/company/full-name phrase so they only fire for a real first name.
  {
    key: 'firstName',
    pattern:
      /(?:first|given|fore)[\s_-]*(?:name|nm)|(?:^|[^a-z])fname|vorname|prenom|voornaam|fornamn|fornavn|etunimi|\bimie\b|\bnombre\b(?!\s*(?:de\b|completo))|\bnome\b(?!\s*(?:completo|utente|de\b|da\b|del))/,
  },
  {
    key: 'lastName',
    pattern:
      /(?:last|family)[\s_-]*(?:name|nm)|(?:^|[^a-z])lname|surname|nachname|familienname|nom de famille|\bapellidos?\b|cognome|achternaam|nazwisko|apelido|sobrenome|efternamn|efternavn|etternavn|sukunimi/,
  },
  // `city`/`town` are anchored on their LEADING side (see the `phone` note
  // above): bare `city` ⊂ "ethnicity", so an EEO "Ethnicity" field was resolving
  // to `location` and receiving the user's city. The compound prefixes
  // (`home`/`work`) are ENUMERATED rather than allowing any letter before `city`,
  // so a camelCase compound flattens and still matches (`workCity` → `workcity`
  // → `work`+`city`) WITHOUT re-opening the `ethnicity` mis-fill (its `city` is
  // preceded by an un-enumerated `ethni`). `hometown` is spelled out so it keeps
  // matching. The trailing side stays open so `city_1` / `cityName` still
  // match. The other EU city/place terms use `\b` where they are
  // short/collision-prone, unchanged.
  {
    key: 'location',
    pattern:
      /(?:^|[^a-z])(?:home|work)?(?:city|town)|\blocation\b|\bort\b|stadt|wohnort|\bville\b|ciudad|citta|plaats|miasto|cidade|localidad/,
  },
];

/**
 * @param signal the full {@link textSignal} (attributes + prose).
 * @param attributes the narrower {@link attributeSignal}; defaults to `signal`,
 *   which is the pre-split behavior — every DOM caller passes it explicitly, and
 *   only a row's {@link NamedKeyPattern.denyAttribute} reads it.
 */
export function matchNamedKey(signal: string, attributes: string = signal): string | null {
  for (const { key, pattern, deny, denyAttribute } of NAMED_KEY_PATTERNS) {
    if (!pattern.test(signal)) continue;
    if (deny?.test(signal)) continue;
    if (denyAttribute?.test(attributes)) continue;
    return key;
  }
  // Generic catch-all: a bare "Name" field → full name, UNLESS the name belongs
  // to a non-person owner ({@link NON_PERSON_NAME_OWNER}) — a school, a company,
  // or a user account.
  if (/\bname\b/.test(signal) && !NON_PERSON_NAME_OWNER.test(signal)) return 'fullName';

  return null;
}
