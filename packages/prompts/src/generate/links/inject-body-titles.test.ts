import { describe, expect, it } from 'vitest';

import { injectLinksIntoGeneratedText } from '../index';

/** Body-link-only injection (no contact links). */
const injectBody = (text: string, bodyMap: Record<string, string>): string =>
  injectLinksIntoGeneratedText(text, {}, bodyMap);

describe('injectLinksIntoGeneratedText', () => {
  describe('body-link title matching (#B/#C — real name, not the machine label)', () => {
    it('links a real-name title against a dashed-slug label', () => {
      const text = ['PROJECTS', 'AI Job Hunter'].join('\n');
      const out = injectBody(text, { 'ai-job-hunter-app': 'https://aijobhunter.app' });
      expect(out).toContain('[AI Job Hunter](https://aijobhunter.app)');
    });

    it('links the same real-name title against the humanised PDF-extraction label', () => {
      // The actual bug case: pdf.rs falls back to the raw URL as anchor text, so
      // bodyLabel() humanises "ai-job-hunter-app" into "ai job hunter app".
      const text = ['PROJECTS', 'AI Job Hunter'].join('\n');
      const out = injectBody(text, { 'ai job hunter app': 'https://aijobhunter.app' });
      expect(out).toContain('[AI Job Hunter](https://aijobhunter.app)');
    });

    it('does not cross-link on a coincidental overlap AT the 6-char floor — declines to pair when two open slots make it ambiguous, and appends instead (#MEDIUM-1/#HIGH part 2, pinned)', () => {
      // "Gotham" (6 chars) clears the floor, but the walk diverges right after
      // ("burg" vs "city…") — a real match requires the full label OR the full
      // line title to be consumed, never just enough chars to clear the floor.
      // A SECOND untouched, item-shaped PROJECTS line keeps the last-resort
      // net's exactly-one-slot pairing from kicking in, so this isolates the
      // cross-link guard from the "never silently drop" guarantee.
      const text = ['PROJECTS', 'Gothamburg Transit Map', 'Some Other Untouched Project'].join(
        '\n'
      );
      const out = injectBody(text, { 'gotham city guide': 'https://example.com/wrong' });
      expect(out).toBe(`${text}\n[gotham city guide](https://example.com/wrong)`);
    });

    it('pairs the sole unmatched label with the sole open item-shaped slot — the actual pairing path, pinned (#HIGH part 2)', () => {
      const text = ['PROJECTS', 'Some Other Project'].join('\n');
      expect(() =>
        injectBody(text, { 'Untouched Project': 'https://example.com/x' })
      ).not.toThrow();
      const out = injectBody(text, { 'Untouched Project': 'https://example.com/x' });
      expect(out).toBe('PROJECTS\n[Some Other Project](https://example.com/x)');
    });

    // Security re-review (MEDIUM, round 7): an item-shaped line can still be
    // a TITLE plus an inline description on the same line — wrapping the
    // whole remainder swallowed the description into the clickable link
    // text. The title becomes the link; the separator + description survive
    // verbatim, unlinked, right after it.
    it('cuts the sole-pairing link at a same-line " — " description separator, preserving the description as plain text', () => {
      const text = ['PROJECTS', 'Orbital Simulator — A physics engine for Unity'].join('\n');
      const out = injectBody(text, { 'orbit-sim': 'https://github.com/jane/orbit-sim' });
      expect(out).toBe(
        'PROJECTS\n[Orbital Simulator](https://github.com/jane/orbit-sim) — A physics engine for Unity'
      );
    });

    it('trims a trailing stray asterisk before wrapping the sole-pairing link, never leaving unbalanced bold', () => {
      const text = ['PROJECTS', 'Orbital Simulator *'].join('\n');
      const out = injectBody(text, { 'orbit-sim': 'https://github.com/jane/orbit-sim' });
      expect(out).toBe('PROJECTS\n[Orbital Simulator](https://github.com/jane/orbit-sim)');
    });

    it('preserves a genuinely balanced bold title intact when sole-pairing, never stripping its legitimate closing **', () => {
      const text = ['PROJECTS', '**Orbital Simulator**'].join('\n');
      const out = injectBody(text, { 'orbit-sim': 'https://github.com/jane/orbit-sim' });
      expect(out).toBe('PROJECTS\n[**Orbital Simulator**](https://github.com/jane/orbit-sim)');
    });

    // CodeRabbit (test-coverage re-review): the sibling test above covers
    // the EVEN-count case ("**Orbital Simulator**" — one opening pair, one
    // legitimate closing pair, must survive). This covers the ODD-count
    // case — a trailing `**` with no matching open anywhere (a stray marker,
    // not a real bold span) — which must still be trimmed, the same way a
    // stray single `*` already is.
    it('trims a trailing STRAY ** (odd pair count, not a legitimate close) before wrapping the sole-pairing link', () => {
      const text = ['PROJECTS', 'Orbital Simulator **'].join('\n');
      const out = injectBody(text, { 'orbit-sim': 'https://github.com/jane/orbit-sim' });
      expect(out).toBe('PROJECTS\n[Orbital Simulator](https://github.com/jane/orbit-sim)');
    });

    it('leaves a link unplaced — not appended, not fabricated — when no PROJECTS/PUBLICATIONS section exists at all (#HIGH part 2)', () => {
      const text = 'Just a summary paragraph with no sections.';
      const out = injectBody(text, { 'Untouched Project': 'https://example.com/x' });
      expect(out).toBe(text);
    });

    // Security re-review (HIGH-4): `detectSections`' own boundary detection
    // (`matchesHeaderTerm` in context-manager/sections.ts) is a lexicon
    // PREFIX match, not a standalone-heading check — "research" is a
    // Publications lexicon term, so a "Research Assistant, Acme Labs" job
    // title (starts with "research" + a space) used to be misdetected as a
    // Publications section boundary. The net then spliced the unmatched
    // label right after that job's own bullet — fabricated content in the
    // EXPERIENCE section, the exact class this file closes twice already.
    // Gating section detection on the real standalone-heading predicates
    // (isKnownSectionName / isAllCapsSectionHeading) rejects the phantom
    // section entirely, so the link is left unplaced instead.
    it('does not treat a "Research …" job title as a Publications section boundary (#HIGH-4)', () => {
      const text = [
        'EXPERIENCE',
        'Research Assistant, Acme Labs',
        '- Studied materials science under Dr. Smith',
      ].join('\n');
      const out = injectBody(text, { 'Untouched Project': 'https://example.com/x' });
      expect(out).toBe(text);
    });

    // Security re-review (HIGH-3, round 7): `matchLineTitle` accepts a match
    // once the LINE (not the label) is fully consumed, as long as it's a
    // genuine (>= MIN_TITLE_KEY_LEN) prefix of the label key — deliberate,
    // for a short renamed item ("orbit-sim" → "Orbital Simulator"). Without a
    // bound excluding the header block, a body label that happens to START
    // WITH the candidate's own name (a project plausibly named after them,
    // "Jane Doe Consulting") could match the header's OWN name line, and the
    // injector would wrap the candidate's name itself in a project
    // hyperlink. The candidate scan is now bounded to lines at/after the
    // first detected section heading, so the header block is never even
    // considered.
    it("never wraps the résumé's own name in a project hyperlink, even when a body label starts with it (#HIGH-3)", () => {
      const text = [
        'Jane Doe',
        'jane@example.com',
        '',
        'EXPERIENCE',
        'Software Engineer, Acme Corp',
        '',
        'PROJECTS',
        'Some Other Project',
      ].join('\n');
      const out = injectBody(text, { 'Jane Doe Consulting': 'https://example.com/consulting' });
      // Byte-for-byte, and asserting WHERE the link actually landed — not
      // just that the header survived. A presence-only assertion is what let
      // the link-fabrication class of bug through four separate times on
      // this branch; it would not have caught a fifth (a mismatch that
      // duplicates the link, or attaches it to the wrong line elsewhere).
      expect(out).toBe(
        [
          'Jane Doe',
          'jane@example.com',
          '',
          'EXPERIENCE',
          'Software Engineer, Acme Corp',
          '',
          'PROJECTS',
          '[Some Other Project](https://example.com/consulting)',
        ].join('\n')
      );
    });

    it('never pairs a description bullet of an already-linked project as an open slot — the bullet stays untouched, the label appends safely instead (#HIGH-1)', () => {
      const text = [
        'PROJECTS',
        '[Fleet Tracker](https://x.dev/fleet)',
        '• Built with Rust and React, deployed to 3 regions',
      ].join('\n');
      const out = injectBody(text, { 'orbit-sim': 'https://github.com/jane/orbit-sim' });
      expect(out).toBe(`${text}\n[orbit-sim](https://github.com/jane/orbit-sim)`);
    });

    // Security re-review (MEDIUM-6): a single top-level bullet marker is now
    // stripped before the shape test, not a blanket rejection — many résumés
    // format project TITLES themselves as a flat bulleted list, not just
    // their descriptions, so the old "any marker = reject" rule made this
    // (common) shape unreachable for the sole-pairing path.
    it('reaches a bulleted project TITLE as an open slot — the marker survives, only the title gets linked (#MEDIUM-6)', () => {
      const text = ['PROJECTS', '- Orbital Simulator'].join('\n');
      const out = injectBody(text, { 'orbit-sim': 'https://github.com/jane/orbit-sim' });
      expect(out).toBe('PROJECTS\n- [Orbital Simulator](https://github.com/jane/orbit-sim)');
    });

    it('still rejects a NESTED/indented sub-bullet as an open slot — only the top-level marker is stripped, not sub-point indentation (#MEDIUM-6)', () => {
      // If indentation weren't rejected, BOTH lines below would count as open
      // slots (2, not 1), so the exactly-one-slot pairing condition would
      // never fire and the label would append as a new item instead of
      // pairing with the top-level title — this differential is what
      // actually proves the indentation check works. Sole-pairing wraps the
      // LINE'S OWN text (the model's real title), not the label — "Orbital
      // Simulator" surviving verbatim is the point (the renamed-item case).
      const text = ['PROJECTS', '- Orbital Simulator', '  - Built with Rust'].join('\n');
      const out = injectBody(text, { 'Untouched Project': 'https://example.com/x' });
      expect(out).toBe(
        'PROJECTS\n- [Orbital Simulator](https://example.com/x)\n  - Built with Rust'
      );
    });

    it('locates a non-English PROJEKTE section via SECTION_LEXICON, not an English-only regex (#HIGH-2)', () => {
      const text = ['PROJEKTE', 'Ein anderes Projekt'].join('\n');
      const out = injectBody(text, { 'orbit-sim': 'https://example.com/orbit-sim' });
      expect(out).toBe('PROJEKTE\n[Ein anderes Projekt](https://example.com/orbit-sim)');
    });

    it('assigns each sibling label its own line instead of first-match-wins swapping URLs (#HIGH-1)', () => {
      // The exact two-item shape prompt B now demands: a repo and its own live
      // site, named for what each one is. First-match-wins used to attach the
      // wrong URL to the wrong item.
      const text = ['PROJECTS', 'CrossKit', 'CrossKit Web'].join('\n');
      const out = injectBody(text, {
        'crosskit web': 'https://example.com/crosskit-web',
        crosskit: 'https://example.com/crosskit',
      });
      expect(out).toContain('[CrossKit](https://example.com/crosskit)');
      expect(out).toContain('[CrossKit Web](https://example.com/crosskit-web)');
      // The bug swapped these — assert the wrong pairing never appears.
      expect(out).not.toContain('[CrossKit](https://example.com/crosskit-web)');
      expect(out).not.toContain('[CrossKit Web](https://example.com/crosskit)');
    });

    it('does not link a bare section-header line or wrap a full prose sentence — pinned end-to-end output (#MEDIUM-1/#HIGH-1/#HIGH part 2)', () => {
      const text = ['PROJECTS', 'Machine learning toolkits are the core of my recent work.'].join(
        '\n'
      );
      const out = injectBody(text, {
        'projects 2024': 'https://example.com/wrong-header',
        'machine-learning-toolkit': 'https://example.com/wrong-prose',
      });
      // Two labels — the last-resort net's single-pairing heuristic never
      // applies — so both append as their own items; the header and the
      // sentence itself are never touched.
      expect(out).toBe(
        `${text}\n[projects 2024](https://example.com/wrong-header)\n[machine-learning-toolkit](https://example.com/wrong-prose)`
      );
    });

    it('is idempotent under a second invocation — no swapped or duplicated links (#MEDIUM-2)', () => {
      const text = ['PROJECTS', 'CrossKit', 'CrossKit Web'].join('\n');
      const map = {
        'crosskit web': 'https://example.com/crosskit-web',
        crosskit: 'https://example.com/crosskit',
      };
      const once = injectBody(text, map);
      const twice = injectBody(once, map);
      expect(twice).toBe(once);
    });

    it('the literal fallback reaches a SHORT key the title matcher cannot — the one case buildBodyLinksBlock still asks the model to echo verbatim (#HIGH part 1/#M7)', () => {
      // "Demo" normalizes to a 4-char key, below MIN_TITLE_KEY_LEN — the
      // title matcher can never reach it (by design), so this is the case
      // the literal fallback actually exists for now, not a general escape
      // hatch for renamed items (that's the last-resort net, tested below).
      const text = 'I built the **Demo** as a side project last year.';
      const out = injectBody(text, { Demo: 'https://example.com/demo' });
      expect(out).toContain('[Demo](https://example.com/demo)');
    });

    it('a digit-leading real-name title matches its slug label — the leading digit is not eaten as a list marker (#M1)', () => {
      const text = ['PROJECTS', '3D Printing Pipeline'].join('\n');
      const out = injectBody(text, { '3d-printing-pipeline': 'https://example.com/3d-print' });
      expect(out).toContain('[3D Printing Pipeline](https://example.com/3d-print)');
    });

    it('a punctuated real-name title matches its slug label — punctuation is an insignificant separator, not just hyphen/underscore/space (#M2)', () => {
      const text = ['PROJECTS', "Jane's Portfolio", 'CrossKit (v2)', 'CrossKit: The Toolkit'].join(
        '\n'
      );
      const out = injectBody(text, {
        'janes-portfolio': 'https://example.com/janes',
        'crosskit-v2': 'https://example.com/v2',
        'crosskit-the-toolkit': 'https://example.com/toolkit',
      });
      expect(out).toContain("[Jane's Portfolio](https://example.com/janes)");
      expect(out).toContain('[CrossKit (v2)](https://example.com/v2)');
      expect(out).toContain('[CrossKit: The Toolkit](https://example.com/toolkit)');
    });

    it('never wraps a span, or pairs a slot, containing `[`/`]` — the widened separator class must not let a match skip over brackets (#MEDIUM)', () => {
      const text = ['PROJECTS', 'CrossKit [beta] Toolkit'].join('\n');
      const out = injectBody(text, { 'crosskit-beta-toolkit': 'https://example.com/beta' });
      // Never nested/broken markdown — the line is left exactly as written
      // (excluded from the last-resort net's slot pool too), and the link
      // is appended as its own clean item instead.
      expect(out).toBe(
        'PROJECTS\nCrossKit [beta] Toolkit\n[crosskit-beta-toolkit](https://example.com/beta)'
      );
    });

    it('a sub-3-char label ("Go") is not silently dropped at intake — it reaches the last-resort net instead of risking the literal fallback on arbitrary prose (#MEDIUM)', () => {
      const text = ['PROJECTS', 'Some Other Project'].join('\n');
      const out = injectBody(text, { Go: 'https://go.dev/x/y' });
      expect(out).toBe('PROJECTS\n[Some Other Project](https://go.dev/x/y)');
    });

    it('an empty model output never gets a fabricated append — no section is ever detected, so nothing is placed (#LOW, resolved as a side effect of #HIGH-2)', () => {
      const out = injectBody('', { 'orbit-sim': 'https://example.com/orbit-sim' });
      expect(out).toBe('');
    });

    it('survives end-to-end for all eight realistic PDF-anchor shapes (#HIGH — the 7-of-8 drop repro)', () => {
      // Five SHORT keys (echoed verbatim per buildBodyLinksBlock's partition,
      // caught by the literal fallback), one renamed item ("orbit-sim" →
      // "Orbital Simulator", caught only by the last-resort net), one
      // digit-leading title (#M1), one trivial exact match.
      const text = [
        'Jane Dev',
        'Engineer',
        'Berlin | jane@example.com',
        '',
        'PROJECTS',
        'Demo',
        'Live',
        'Paper',
        'PDF',
        'GitHub',
        'Orbital Simulator',
        '3D Printing Pipeline',
        'CrossKit',
      ].join('\n');
      const bodyMap = {
        Demo: 'https://example.com/demo',
        Live: 'https://example.com/live',
        Paper: 'https://example.com/paper',
        PDF: 'https://example.com/pdf',
        GitHub: 'https://example.com/gh',
        'orbit-sim': 'https://example.com/orbit-sim',
        '3d-printing-pipeline': 'https://example.com/3d-print',
        CrossKit: 'https://example.com/crosskit',
      };
      const out = injectBody(text, bodyMap);
      for (const url of Object.values(bodyMap)) {
        expect(out).toContain(`](${url})`);
      }
    });

    it('folds accents so an accented real-name title matches its ASCII slug label (#MEDIUM-4)', () => {
      const text = ['PROJECTS', 'Café Münster Planner'].join('\n');
      const out = injectBody(text, { 'cafe-munster-planner': 'https://example.com/cafe' });
      expect(out).toContain('[Café Münster Planner](https://example.com/cafe)');
    });

    it('never emits an unpaired UTF-16 surrogate for adjacent astral-plane characters (#MEDIUM-3)', () => {
      const text = ['PROJECTS', 'Rocketry \u{1D550}Lab'].join('\n'); // 𝕐
      const out = injectBody(
        text,
        { 'Rocketry \u{1D54F}Lab': 'https://example.com/x' } // 𝕏 — differs only in the low surrogate
      );
      // Regardless of whether this coincidentally matches, the output string
      // must stay well-formed UTF-16 (encodeURIComponent throws on a lone
      // surrogate, mirroring the serde_json rejection this guards against).
      expect(() => encodeURIComponent(out)).not.toThrow();
    });
  });
});
