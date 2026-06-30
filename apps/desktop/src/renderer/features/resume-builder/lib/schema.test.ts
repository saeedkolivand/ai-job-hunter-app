/**
 * builderSchema — Zod validation tests.
 *
 * Covers:
 *  - A fully-valid answers object passes.
 *  - project/publication `link`: non-URL fails on the correct path; valid http/https passes.
 *  - `year`: non-4-digit fails; 4-digit passes; blank is fine.
 *  - experience identity refinement: title+company both blank → fails on `title`; either
 *    present → passes; fully blank entry passes (not "touched").
 *  - education identity refinement: degree+institution both blank → fails on `degree`; either
 *    present → passes; fully blank entry passes.
 *  - Empty/blank optional strings don't error.
 */
import { describe, expect, it } from 'vitest';

import { builderSchema } from './schema';

// ── helpers ───────────────────────────────────────────────────────────────────

function makeBlankExperience() {
  return {
    title: '',
    company: '',
    location: '',
    startDate: '',
    endDate: '',
    current: false,
    bullets: [],
  };
}

function makeBlankEducation() {
  return {
    degree: '',
    institution: '',
    location: '',
    startDate: '',
    endDate: '',
    details: '',
  };
}

function makeBlankEntry() {
  return { title: '', detail: '', year: '' };
}

/** Full valid answers object — every field present but empty where optional. */
function validAnswers() {
  return {
    headline: 'Senior Engineer',
    summary: 'Experienced developer.',
    experience: [
      {
        title: 'Engineer',
        company: 'Acme',
        location: 'Berlin',
        startDate: 'Jan 2020',
        endDate: 'Dec 2023',
        current: false,
        bullets: ['Built things'],
      },
    ],
    education: [
      {
        degree: 'BSc',
        institution: 'MIT',
        location: 'Boston',
        startDate: 'Sep 2015',
        endDate: 'Jun 2019',
        details: '',
      },
    ],
    skills: ['TypeScript', 'Rust'],
    projects: [{ name: 'My App', description: 'Cool app.', link: 'https://example.com' }],
    publications: [{ title: 'Paper', venue: 'ICML', year: '2022', link: '' }],
    awards: [{ title: 'Best Dev', detail: 'Won it', year: '2023' }],
    volunteer: [makeBlankEntry()],
    languages: ['English'],
    certifications: ['AWS'],
  };
}

// ── full-valid pass ───────────────────────────────────────────────────────────

describe('builderSchema — full-valid object', () => {
  it('passes safeParse for a complete, valid answers object', () => {
    expect(builderSchema.safeParse(validAnswers()).success).toBe(true);
  });
});

// ── urlField (project.link / publication.link) ────────────────────────────────

describe('builderSchema — project link (urlField)', () => {
  it('fails when link is a bare string (non-URL)', () => {
    const data = validAnswers();
    data.projects = [{ name: 'P', description: '', link: 'not-a-url' }];
    const result = builderSchema.safeParse(data);
    expect(result.success).toBe(false);
    if (!result.success) {
      const paths = result.error.issues.map((i) => i.path.join('.'));
      expect(paths.some((p) => p.includes('link'))).toBe(true);
      expect(result.error.issues[0]?.message).toBe('build.validation.url');
    }
  });

  it('passes when project link is a valid https URL', () => {
    const data = validAnswers();
    data.projects = [{ name: 'P', description: '', link: 'https://github.com/user/repo' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when project link is a valid http URL', () => {
    const data = validAnswers();
    data.projects = [{ name: 'P', description: '', link: 'http://example.com' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when project link is empty (optional)', () => {
    const data = validAnswers();
    data.projects = [{ name: 'P', description: '', link: '' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });
});

describe('builderSchema — publication link (urlField)', () => {
  it('fails when publication link is a non-URL string', () => {
    const data = validAnswers();
    data.publications = [{ title: 'T', venue: 'V', year: '2021', link: 'ftp://bad' }];
    const result = builderSchema.safeParse(data);
    expect(result.success).toBe(false);
    if (!result.success) {
      const paths = result.error.issues.map((i) => i.path.join('.'));
      expect(paths.some((p) => p.includes('link'))).toBe(true);
    }
  });

  it('passes when publication link is blank', () => {
    const data = validAnswers();
    data.publications = [{ title: 'T', venue: 'V', year: '2021', link: '' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });
});

// ── yearField ─────────────────────────────────────────────────────────────────

describe('builderSchema — year field (yearField)', () => {
  it('fails when publication year is not 4 digits', () => {
    const data = validAnswers();
    data.publications = [{ title: 'T', venue: 'V', year: '22', link: '' }];
    const result = builderSchema.safeParse(data);
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues[0]?.message).toBe('build.validation.year');
      const paths = result.error.issues.map((i) => i.path.join('.'));
      expect(paths.some((p) => p.includes('year'))).toBe(true);
    }
  });

  it('fails when award year has non-digit chars', () => {
    const data = validAnswers();
    data.awards = [{ title: 'A', detail: '', year: '202X' }];
    const result = builderSchema.safeParse(data);
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues[0]?.message).toBe('build.validation.year');
    }
  });

  it('passes when year is exactly 4 digits', () => {
    const data = validAnswers();
    data.publications = [{ title: 'T', venue: 'V', year: '2024', link: '' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when year is blank (optional)', () => {
    const data = validAnswers();
    data.publications = [{ title: 'T', venue: 'V', year: '', link: '' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when year is whitespace-only (treated as blank)', () => {
    const data = validAnswers();
    data.awards = [{ title: 'A', detail: '', year: '   ' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });
});

// ── experience identity refinement ────────────────────────────────────────────

describe('builderSchema — experience identity refinement', () => {
  it('fails on `title` path when a touched entry has no title AND no company', () => {
    const data = validAnswers();
    // location is filled → "touched"; title and company are blank → identity error
    data.experience = [{ ...makeBlankExperience(), location: 'Berlin' }];
    const result = builderSchema.safeParse(data);
    expect(result.success).toBe(false);
    if (!result.success) {
      const paths = result.error.issues.map((i) => i.path.join('.'));
      expect(paths.some((p) => p.endsWith('title'))).toBe(true);
      expect(result.error.issues[0]?.message).toBe('build.validation.experienceIdentity');
    }
  });

  it('passes when the entry has a title (no company required)', () => {
    const data = validAnswers();
    data.experience = [{ ...makeBlankExperience(), title: 'Engineer', location: 'Berlin' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when the entry has a company (no title required)', () => {
    const data = validAnswers();
    data.experience = [{ ...makeBlankExperience(), company: 'Acme', location: 'Berlin' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when the entry is fully blank (not touched)', () => {
    const data = validAnswers();
    data.experience = [makeBlankExperience()];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when the experience array is empty', () => {
    const data = validAnswers();
    data.experience = [];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });
});

// ── education identity refinement ─────────────────────────────────────────────

describe('builderSchema — education identity refinement', () => {
  it('fails on `degree` path when a touched entry has no degree AND no institution', () => {
    const data = validAnswers();
    // location is filled → "touched"; degree and institution blank → identity error
    data.education = [{ ...makeBlankEducation(), location: 'Berlin' }];
    const result = builderSchema.safeParse(data);
    expect(result.success).toBe(false);
    if (!result.success) {
      const paths = result.error.issues.map((i) => i.path.join('.'));
      expect(paths.some((p) => p.endsWith('degree'))).toBe(true);
      expect(result.error.issues[0]?.message).toBe('build.validation.educationIdentity');
    }
  });

  it('passes when the entry has a degree (no institution required)', () => {
    const data = validAnswers();
    data.education = [{ ...makeBlankEducation(), degree: 'BSc', location: 'Boston' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when the entry has an institution (no degree required)', () => {
    const data = validAnswers();
    data.education = [{ ...makeBlankEducation(), institution: 'MIT', location: 'Boston' }];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when the education entry is fully blank (not touched)', () => {
    const data = validAnswers();
    data.education = [makeBlankEducation()];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when the education array is empty', () => {
    const data = validAnswers();
    data.education = [];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });
});

// ── empty/blank optional top-level fields ─────────────────────────────────────

describe('builderSchema — empty/blank optional fields', () => {
  it('passes when headline and summary are empty strings', () => {
    const data = validAnswers();
    data.headline = '';
    data.summary = '';
    expect(builderSchema.safeParse(data).success).toBe(true);
  });

  it('passes when skills, languages, certifications are empty arrays', () => {
    const data = validAnswers();
    data.skills = [];
    data.languages = [];
    data.certifications = [];
    expect(builderSchema.safeParse(data).success).toBe(true);
  });
});
