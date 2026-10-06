import { describe, expect, it } from 'vitest';

import urlLabels from '../../fixtures/url-labels.json';
import {
  buildBodyLinksBlock,
  getBodyLinkMap,
  getLinkMap,
  injectLinksIntoGeneratedText,
  parseLinksFromResume,
  urlToFriendlyLabel,
} from '../index';
import { RESUME_WITH_LINKS } from '../test-support';

describe('getLinkMap', () => {
  it('maps profile labels to URLs, drops email, and admits one Website link', () => {
    const map = getLinkMap(RESUME_WITH_LINKS);
    expect(map.LinkedIn).toBe('https://linkedin.com/in/johndoe');
    expect(map.GitHub).toBe('https://github.com/johndoe');
    expect(map.Email).toBeUndefined();
    // The single non-platform URL is admitted under a generic "Website" label —
    // never under its raw anchor text.
    expect(map.Website).toBe('https://not-a-profile.example.com');
    expect(map.Personal).toBeUndefined();
  });

  it('admits exactly one Website link; later non-platform URLs become body links (#18)', () => {
    const resume = [
      'Body',
      '---',
      '- [LinkedIn](https://linkedin.com/in/jane)',
      '- [Portfolio](https://janedoe.dev)',
      '- [Blog](https://janeblog.example)',
      '- [Email](mailto:jane@example.com)',
    ].join('\n');
    const map = getLinkMap(resume);
    expect(map.LinkedIn).toBe('https://linkedin.com/in/jane');
    expect(map.Website).toBe('https://janedoe.dev'); // first bare-root non-platform wins
    expect(Object.values(map)).not.toContain('https://janeblog.example'); // 2nd → body, not contact
    expect(Object.values(map)).not.toContain('mailto:jane@example.com'); // mailto dropped
    // The second personal site is no longer dropped — it is preserved as a body link.
    expect(getBodyLinkMap(resume).Blog).toBe('https://janeblog.example');
  });

  // LOW (security re-review): the Website pre-pass's scheme check is
  // case-sensitive — matches Rust's mirrored `classify_contact_links`, which
  // uses a plain `starts_with("http://") || starts_with("https://")` with no
  // lowercasing. A case-insensitive check would admit an uppercase-scheme
  // candidate Rust's own pre-pass never would.
  it('does not admit an uppercase-scheme URL to the Website slot (case-sensitive, matches Rust)', () => {
    const resume = ['Body', '---', '- [Portfolio](HTTPS://janedoe.dev)'].join('\n');
    const map = getLinkMap(resume);
    expect(map.Website).toBeUndefined();
  });

  it('returns an empty map when there is no reference block', () => {
    expect(getLinkMap('Just a plain resume with no separator')).toEqual({});
  });

  it('derives a friendly label when the anchor is a raw URL', () => {
    const resume = `Body\n---\n- [https://github.com/jane](https://github.com/jane)`;
    const map = getLinkMap(resume);
    expect(map.GitHub).toBe('https://github.com/jane');
  });

  it('keeps a GitHub profile (one path segment) on the contact line', () => {
    const map = getLinkMap('Body\n---\n- [GitHub](https://github.com/jane)');
    expect(map.GitHub).toBe('https://github.com/jane');
    expect(getBodyLinkMap('Body\n---\n- [GitHub](https://github.com/jane)')).toEqual({});
  });
});

describe('getBodyLinkMap (#18 — body links)', () => {
  it('classifies project / publication / repo links as body, not contact', () => {
    const resume = [
      'Body',
      '---',
      '- [LinkedIn](https://linkedin.com/in/jane)', // contact profile
      '- [GitHub](https://github.com/jane)', // contact profile (1 segment)
      '- [orbit-sim](https://github.com/jane/orbit-sim)', // deep repo → body
      '- [Spin glasses in 2D](https://doi.org/10.1103/PhysRevB.1.234)', // publication → body
      '- [Email](mailto:jane@example.com)',
    ].join('\n');

    const contact = getLinkMap(resume);
    expect(contact.LinkedIn).toBe('https://linkedin.com/in/jane');
    expect(contact.GitHub).toBe('https://github.com/jane');

    const body = getBodyLinkMap(resume);
    expect(body['orbit-sim']).toBe('https://github.com/jane/orbit-sim');
    expect(body['Spin glasses in 2D']).toBe('https://doi.org/10.1103/PhysRevB.1.234');
    // The repo did NOT pollute the contact map, and the profile is not a body link.
    expect(contact['orbit-sim']).toBeUndefined();
    expect(body.GitHub).toBeUndefined();
  });

  it('humanises a slug when a body link anchor is a raw URL (PDF case)', () => {
    const resume =
      'Body\n---\n- [https://example.org/my-research-paper](https://example.org/my-research-paper)';
    expect(getBodyLinkMap(resume)['my research paper']).toBe(
      'https://example.org/my-research-paper'
    );
  });

  it('returns an empty map when there are no body links', () => {
    expect(getBodyLinkMap(RESUME_WITH_LINKS)).toEqual({});
  });
});

describe('classifyLinks — apex-over-subdomain Website preference (#A parity)', () => {
  it('prefers the apex host over its own subdomain, regardless of input order', () => {
    const subFirst = [
      'Body',
      '---',
      '- [Blog](https://blog.example.dev)',
      '- [Site](https://example.dev)',
    ].join('\n');
    expect(getLinkMap(subFirst).Website).toBe('https://example.dev');

    const apexFirst = [
      'Body',
      '---',
      '- [Site](https://example.dev)',
      '- [Blog](https://blog.example.dev)',
    ].join('\n');
    expect(getLinkMap(apexFirst).Website).toBe('https://example.dev');
  });

  it('resolves a 3-level chain to the true apex', () => {
    const resume = [
      'Body',
      '---',
      '- [A](https://a.b.c.dev)',
      '- [B](https://b.c.dev)',
      '- [C](https://c.dev)',
    ].join('\n');
    expect(getLinkMap(resume).Website).toBe('https://c.dev');
  });

  it('treats notexample.dev and example.dev as unrelated apexes (dot-prefix guard)', () => {
    // A naive substring endsWith('example.dev') would wrongly treat
    // "notexample.dev" as a subdomain of "example.dev" — neither is actually
    // a subdomain of the other, so first-seen decides.
    const resume = [
      'Body',
      '---',
      '- [First](https://notexample.dev)',
      '- [Second](https://example.dev)',
    ].join('\n');
    expect(getLinkMap(resume).Website).toBe('https://notexample.dev');
  });

  it('keeps the bare-root candidate that lost the Website slot as a body link', () => {
    const resume = [
      'Body',
      '---',
      '- [Blog](https://blog.example.dev)',
      '- [Site](https://example.dev)',
    ].join('\n');
    expect(getBodyLinkMap(resume).Blog).toBe('https://blog.example.dev');
  });
});

describe('LinkedIn /in/ gate (pre-existing parity with Rust is_personal_linkedin)', () => {
  it('admits a personal LinkedIn profile to the contact line', () => {
    const resume = 'Body\n---\n- [LinkedIn](https://linkedin.com/in/jane)';
    expect(getLinkMap(resume).LinkedIn).toBe('https://linkedin.com/in/jane');
  });

  it('does not admit a LinkedIn company page as a contact link or a fabricated body link — it is dropped entirely (#M6)', () => {
    // A body entry would make buildBodyLinksBlock ask the model to invent a
    // PROJECTS item for an employer's LinkedIn page — mirrors Rust
    // classify_contact_links, which drops these entirely.
    const resume = 'Body\n---\n- [Acme](https://linkedin.com/company/acme)';
    expect(getLinkMap(resume)).toEqual({});
    expect(getBodyLinkMap(resume)).toEqual({});
  });
});

describe('Website apex/first-seen pre-pass parity with Rust (#L1)', () => {
  it('never admits a job-board apex as the Website contact link, or a fabricated body project either (#HIGH-3)', () => {
    const resume = [
      'Body',
      '---',
      '- [Indeed](https://indeed.com)',
      '- [Portfolio](https://janedoe.dev)',
    ].join('\n');
    const map = getLinkMap(resume);
    expect(map.Website).toBe('https://janedoe.dev');
    expect(Object.values(map)).not.toContain('https://indeed.com');
    // The prior fix only kept it off the Website pre-pass — it still fell
    // through to `body`, and buildBodyLinksBlock would ask the model to
    // invent a PROJECTS item named "Indeed" for it (#HIGH-3, the same
    // fabrication risk #M6 closed for non-personal LinkedIn).
    expect(getBodyLinkMap(resume)).toEqual({});
  });

  it('drops a job-board ATS apply link entirely — never a contact link, never a fabricated "Apply" project (#HIGH-3)', () => {
    const resume = 'Body\n---\n- [Apply](https://boards.greenhouse.io/acme/jobs/123)';
    expect(getLinkMap(resume)).toEqual({});
    expect(getBodyLinkMap(resume)).toEqual({});
  });
});

describe('Xing profile gate (#LOW, deliberate — xing.com is also a JOB_BOARD_HOSTS entry)', () => {
  it('admits a personal Xing profile to the contact line', () => {
    const resume = 'Body\n---\n- [Xing](https://www.xing.com/profile/Jane_Doe)';
    expect(getLinkMap(resume).Xing).toBe('https://www.xing.com/profile/Jane_Doe');
  });

  it('drops a Xing job listing entirely — never a contact link, never a fabricated "Xing" project', () => {
    const resume = 'Body\n---\n- [Job](https://www.xing.com/jobs/12345)';
    expect(getLinkMap(resume)).toEqual({});
    expect(getBodyLinkMap(resume)).toEqual({});
  });
});

describe('bio-link platform hosts, matching Rust WEBSITE_HOSTS', () => {
  it('recognizes about.me as a platform host', () => {
    const resume = 'Body\n---\n- [About](https://about.me/janedoe)';
    expect(getLinkMap(resume).About).toBe('https://about.me/janedoe');
  });

  it('recognizes carrd.co as a platform host', () => {
    const resume = 'Body\n---\n- [Site](https://janedoe.carrd.co)';
    expect(getLinkMap(resume).Site).toBe('https://janedoe.carrd.co');
  });
});

describe('uniqueBodyLabel — colliding normalized keys stay distinct (#M4/#M5)', () => {
  it('keeps two anchors that normalize to the same key as two entries with distinct keys, regardless of input order', () => {
    const build = (first: string, second: string) => ['Body', '---', first, second].join('\n');
    const crossKit = '- [CrossKit](https://example.com/crosskit-repo)';
    const crossHyphenKit = '- [Cross-Kit](https://example.org/crosskit-pkg)';

    for (const resume of [build(crossKit, crossHyphenKit), build(crossHyphenKit, crossKit)]) {
      const body = getBodyLinkMap(resume);
      const labels = Object.keys(body);
      expect(labels).toHaveLength(2); // neither URL silently overwrote the other
      expect(Object.values(body)).toEqual(
        expect.arrayContaining([
          'https://example.com/crosskit-repo',
          'https://example.org/crosskit-pkg',
        ])
      );
      // The disambiguator is a plain number, never parens (#M5).
      const suffixed = labels.find((l) => l !== 'CrossKit' && l !== 'Cross-Kit');
      expect(suffixed).toBeDefined();
      expect(suffixed).not.toContain('(');
    }
  });

  it('the numbered disambiguator stays literal-fallback-reachable, unlike the old "(2)" suffix (#M5)', () => {
    // `\b…\b` cannot match a `)`-terminated label — the old suffix made a
    // numbered duplicate unlinkable by any phrasing.
    const resume = [
      'Body',
      '---',
      '- [CrossKit](https://example.com/crosskit-repo)',
      '- [CrossKit](https://example.org/crosskit-second)',
    ].join('\n');
    const body = getBodyLinkMap(resume);
    expect(body['CrossKit 2']).toBe('https://example.org/crosskit-second');
    const out = injectLinksIntoGeneratedText('The second CrossKit 2 tool I built.', {}, body);
    expect(out).toContain('[CrossKit 2](https://example.org/crosskit-second)');
  });
});

describe('parseLinksFromResume', () => {
  it('extracts a clean email, profile labels, and the Website label', () => {
    const { block, cleanEmail } = parseLinksFromResume(RESUME_WITH_LINKS);
    expect(cleanEmail).toBe('john@example.com');
    expect(block).toContain('LinkedIn');
    expect(block).toContain('GitHub');
    expect(block).toContain('Website'); // non-platform URL surfaced for the AI to write
  });

  it('returns empty result when there is no reference block', () => {
    expect(parseLinksFromResume('No block here')).toEqual({ block: '', cleanEmail: '' });
  });
});

describe('buildBodyLinksBlock (#18)', () => {
  it('lists body link labels and instructs the model to keep them on their items', () => {
    const resume = [
      'Body',
      '---',
      '- [LinkedIn](https://linkedin.com/in/jane)',
      '- [orbit-sim](https://github.com/jane/orbit-sim)',
      '- [My thesis](https://doi.org/10.1/x)',
    ].join('\n');
    const block = buildBodyLinksBlock(resume);
    expect(block).toContain('orbit-sim');
    expect(block).toContain('My thesis');
    expect(block).toContain('PROJECTS');
    // Contact-line links must NOT appear in the body block.
    expect(block).not.toContain('LinkedIn');
  });

  it('returns an empty string when there are no body links', () => {
    expect(buildBodyLinksBlock(RESUME_WITH_LINKS)).toBe('');
  });

  it('partitions short (unmatchable) keys into a verbatim-echo instruction, keeping the real-name wording for the rest (#HIGH part 1)', () => {
    const resume = [
      'Body',
      '---',
      '- [Demo](https://example.com/demo)', // 4-char key — below MIN_TITLE_KEY_LEN
      '- [orbit-sim](https://github.com/jane/orbit-sim)', // 8-char key — reachable
    ].join('\n');
    const block = buildBodyLinksBlock(resume);
    expect(block).toContain('SHORT KEYS');
    expect(block).toMatch(/SHORT KEYS[\s\S]*- Demo/);
    expect(block).toMatch(/REAL name[\s\S]*- orbit-sim/);
    // The short-key section is the only place still asking for a verbatim
    // echo — the real-name section still forbids it.
    expect(block).toContain('write EACH ONE exactly as shown below, verbatim');
    expect(block).toContain('never the key itself');
  });
});

describe('urlToFriendlyLabel ↔ Rust url_label parity', () => {
  // Shared source of truth with `cargo test export::links` — both suites read
  // fixtures/url-labels.json so the two implementations can never silently drift.
  it('matches the shared fixture for every URL', () => {
    const cases = urlLabels as { url: string; label: string }[];
    expect(cases.length).toBeGreaterThan(0);
    for (const { url, label } of cases) {
      expect(urlToFriendlyLabel(url)).toBe(label);
    }
  });
});
