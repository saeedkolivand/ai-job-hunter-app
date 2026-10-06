/**
 * Unit tests for the Tier-2 free-text matcher's EU-language labels and the
 * localized / third-party denylist hardening (apps/extension/src/lib/autofill.ts).
 *
 * Each language: native-labelled first/last/email/phone/location fields fill
 * from the profile (proving the widened keyword table AND the accent-free signal
 * normalization — every label below carries real diacritics), while its school /
 * company / username "name" fields do NOT fill (proving the localized
 * generic-name denylist + the first-name negative lookaheads).
 */

import { afterEach, describe, expect, it } from 'vitest';

import { textSignal } from '../field-signal';
import { expectEmpty, expectValues, fill, labelled, resetDocument, setForm } from './test-support';

afterEach(resetDocument);

type Row = [id: string, label: string, type?: string];

const PHONE = '+31612345678';
const CITY = 'Amsterdam, Netherlands';
const EMAIL = 'saeed@example.com';

describe('planAndFill – EU-language field labels (Tier-2 free-text matcher)', () => {
  it('German: Vorname/Nachname/E-Mail-Adresse/Telefonnummer/Wohnort fill', () => {
    const summary = fill(
      labelled([
        ['vn', 'Vorname'],
        ['nn', 'Nachname'],
        ['em', 'E-Mail-Adresse', 'email'],
        ['tp', 'Telefonnummer', 'tel'],
        ['wo', 'Wohnort'],
      ])
    );
    expectValues({ vn: 'Saeed', nn: 'Kolivand', em: EMAIL, tp: PHONE, wo: CITY });
    expect(summary.nameSplit).toEqual({ first: 'Saeed', last: 'Kolivand' });
  });

  it.each([
    [
      // The \b-anchored handy/mobil keywords miss these standard DE compounds, so
      // they are listed explicitly in the phone pattern.
      'German: concatenated mobile labels "Handynummer" / "Mobilnummer" / "Mobiltelefon" fill from the phone value',
      [
        ['h', 'Handynummer', 'tel'],
        ['m', 'Mobilnummer', 'tel'],
        ['mt', 'Mobiltelefon', 'tel'],
      ],
      { h: PHONE, m: PHONE, mt: PHONE },
    ],
    [
      'French: Prénom/Nom de famille/Adresse e-mail/Numéro de téléphone/Ville fill',
      [
        ['pr', 'Prénom'],
        ['nf', 'Nom de famille'],
        ['em', 'Adresse e-mail', 'email'],
        ['tp', 'Numéro de téléphone', 'tel'],
        ['vi', 'Ville'],
      ],
      { pr: 'Saeed', nf: 'Kolivand', em: EMAIL, tp: PHONE, vi: CITY },
    ],
    [
      'Spanish: Nombre/Apellidos/Correo electrónico/Teléfono/Ciudad fill',
      [
        ['nb', 'Nombre'],
        ['ap', 'Apellidos'],
        ['em', 'Correo electrónico', 'email'],
        ['tp', 'Teléfono', 'tel'],
        ['ci', 'Ciudad'],
      ],
      { nb: 'Saeed', ap: 'Kolivand', em: EMAIL, tp: PHONE, ci: CITY },
    ],
    [
      'Polish: Imię/Nazwisko/E-mail/Telefon/Miasto fill',
      [
        ['im', 'Imię'],
        ['nz', 'Nazwisko'],
        ['em', 'E-mail', 'email'],
        ['tp', 'Telefon', 'tel'],
        ['mi', 'Miasto'],
      ],
      { im: 'Saeed', nz: 'Kolivand', em: EMAIL, tp: PHONE, mi: CITY },
    ],
    [
      'Italian: Nome/Cognome/Indirizzo email/Telefono/Città fill (Cognome is NOT read as a first name)',
      [
        ['no', 'Nome'],
        ['co', 'Cognome'],
        ['em', 'Indirizzo email', 'email'],
        ['tp', 'Telefono', 'tel'],
        ['ci', 'Città'],
      ],
      { no: 'Saeed', co: 'Kolivand', em: EMAIL, tp: PHONE, ci: CITY },
    ],
  ] as [string, Row[], Record<string, string>][])('%s', (_name, rows, expected) => {
    fill(labelled(rows));
    expectValues(expected);
  });

  it.each([
    [
      'German: Schulname / "Name der Schule" / "Name des Unternehmens" are NOT filled',
      [
        ['s1', 'Schulname'],
        ['s2', 'Name der Schule'],
        ['c1', 'Firmenname'],
        ['c2', 'Name des Unternehmens'],
      ],
    ],
    [
      // Why the compounds are explicit rather than unanchored `handy`/`mobil`:
      // bare substrings would wrongly fill these real, unrelated labels.
      'German phone stays anchored: "Automobilhersteller" / "Handyman" do NOT match the phone key',
      [
        ['a', 'Automobilhersteller'],
        ['hm', 'Handyman services'],
      ],
    ],
    [
      "French: nom de l'école / nom de l'entreprise are NOT filled",
      [
        ['e1', "Nom de l'école"],
        ['e2', "Nom de l'entreprise"],
      ],
    ],
    [
      // The load-bearing case: `nombre` means "name" so a bare `nombre` pattern
      // would mis-fill both of these; the negative lookahead is what stops it.
      'Spanish: "Nombre de usuario" (username) and "Nombre de la empresa" (company) are NOT filled',
      [
        ['u', 'Nombre de usuario'],
        ['c', 'Nombre de la empresa'],
      ],
    ],
    [
      'Polish: "Nazwa firmy" (company) and "Nazwa użytkownika" (username) are NOT filled',
      [
        ['c', 'Nazwa firmy'],
        ['u', 'Nazwa użytkownika'],
      ],
    ],
    ['Italian: "Nome utente" (username) is NOT filled as a first name', [['u', 'Nome utente']]],
  ] as [string, Row[]][])('%s', (_name, rows) => {
    fill(labelled(rows));
    expectEmpty(...rows.map(([id]) => id));
  });

  it('resolves a COMBINED full-name field to fullName, not just its first/last token', () => {
    // Combined phrases contain the first/last keywords as substrings, so they
    // must be matched BEFORE first/last (else "Nombre completo" would fill with
    // just "Saeed"). Filled as one fullName value — no first/last split.
    const summary = fill(
      labelled([
        ['es', 'Nombre completo'],
        ['pl', 'Imię i nazwisko'],
        ['de', 'Vollständiger Name'],
      ])
    );
    expectValues({ es: 'Saeed Kolivand', pl: 'Saeed Kolivand', de: 'Saeed Kolivand' });
    expect(summary.nameSplit).toBeNull();
    expect(summary.filled.every((f) => f.key === 'fullName')).toBe(true);
  });

  it('fills a combined "first AND last" label as the WHOLE name, not a partial first/last', () => {
    // "Vor- und Nachname" contains "nachname" (→ lastName) and "Nombre y
    // apellidos" contains "nombre" (→ firstName); without the conjunction forms
    // in the fullName pattern each would fill only half the name.
    const summary = fill(
      labelled([
        ['de', 'Vor- und Nachname'],
        ['es', 'Nombre y apellidos'],
        ['it', 'Nome e cognome'],
        ['fr', 'Prénom et nom'],
        ['nl', 'Voor- en achternaam'],
      ])
    );
    expectValues(
      Object.fromEntries(['de', 'es', 'it', 'fr', 'nl'].map((id) => [id, 'Saeed Kolivand']))
    );
    expect(summary.nameSplit).toBeNull();
    expect(summary.filled.every((f) => f.key === 'fullName')).toBe(true);
  });

  it('still skips a localized sensitive-PII field (DE Geburtsdatum, PL PESEL)', () => {
    fill(`
      <label for="g">Geburtsdatum</label><input id="g" type="text" autocomplete="email" />
      <label for="p">Numer PESEL</label><input id="p" type="text" autocomplete="email" />
    `);
    expectEmpty('g', 'p');
  });
});

describe('planAndFill – third-party / localized denylist hardening', () => {
  it('German: "Name des Ansprechpartners / Notfallkontakts / Referenzperson" third-party fields are NOT filled', () => {
    // \bname\b fires on all three, so without the localized ansprechpartner /
    // notfall / referenz denylist they would mis-fill with the applicant's name.
    fill(
      labelled([
        ['a', 'Name des Ansprechpartners'],
        ['n', 'Name des Notfallkontakts'],
        ['r', 'Name der Referenzperson'],
      ])
    );
    expectEmpty('a', 'n', 'r');
  });

  it('French: "Nom du contact d\'urgence" / "Numéro d\'urgence" are NOT filled (urgence is word-anchored)', () => {
    fill(
      labelled([
        ['cu', "Nom du contact d'urgence"],
        ['nu', "Numéro d'urgence", 'tel'],
      ])
    );
    expectEmpty('cu', 'nu');
  });

  it('English: a "Name (Affirmative Action)" EEO field STILL fills — `firma` is word-anchored, not a bare substring', () => {
    // "affirmative" contains the substring "firma"; a bare `firma` denylist term
    // would have wrongly blocked this real US EEO self-identification field.
    fill(labelled([['aa', 'Name (Voluntary Self-Identification — Affirmative Action)']]));
    expectValues({ aa: 'Saeed Kolivand' });
  });

  it('Norwegian: a "Fødselsnummer" field is skipped (ø is folded to o so the denylist matches)', () => {
    // NFD does NOT decompose ø, so without the explicit fold "fodselsnummer"
    // never matches "Fødselsnummer" and this national-id field would fill.
    fill(`<label for="f">Fødselsnummer</label><input id="f" type="text" autocomplete="email" />`);
    expectEmpty('f');
  });

  it('word-anchors "dni": a standalone "DNI" field is skipped, but Polish "poprzedni" (previous) does not over-skip a first-name field', () => {
    fill(`
      <label for="d1">Número de DNI</label><input id="d1" type="text" autocomplete="email" />
      <label for="im2">Imię</label><input id="im2" name="poprzedni_krok" type="text" />
    `);
    // "DNI" whole word → ambiguous, skipped; "poprzedni" substring must NOT trigger the dni rule
    expectValues({ d1: '', im2: 'Saeed' });
  });

  it('folds atomic non-decomposable letters (ø/æ/ł/đ/ß) that NFD leaves intact', () => {
    setForm(labelled([['x', 'Fødselsnummer Szkoła Æther Đ Straße']]));
    const signal = textSignal(document.getElementById('x') as HTMLElement);
    expect(signal).toContain('fodselsnummer');
    expect(signal).toContain('szkola');
    expect(signal).toContain('aether');
    expect(signal).toContain('strasse');
    expect(signal).toContain(' d '); // Đ → d
  });
});
