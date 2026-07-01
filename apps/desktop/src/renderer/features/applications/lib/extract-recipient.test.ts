import { describe, expect, it } from 'vitest';

import { extractRecipient } from './extract-recipient';

describe('extractRecipient', () => {
  it('extracts name and email from a "send CV to Name <email>" pattern', () => {
    const result = extractRecipient('Please send your CV to Jane Doe <jane@acme.com> by Friday.');
    expect(result.email).toBe('jane@acme.com');
    expect(result.name).toBe('Jane Doe');
  });

  it('extracts name from "contact Name at email" pattern', () => {
    const result = extractRecipient(
      'To apply, contact John Smith at john.smith@example.com with your resume.'
    );
    expect(result.email).toBe('john.smith@example.com');
    expect(result.name).toBe('John Smith');
  });

  it('returns empty object when no email is present', () => {
    const result = extractRecipient(
      'Send your resume to our HR team via the portal. No direct emails please.'
    );
    expect(result.email).toBeUndefined();
    expect(result.name).toBeUndefined();
  });

  it('returns email without name when no nearby capitalized name exists', () => {
    const result = extractRecipient('Email hr@company.com for more info.');
    expect(result.email).toBe('hr@company.com');
    expect(result.name).toBeUndefined();
  });

  it('handles an empty string input', () => {
    expect(extractRecipient('')).toEqual({});
  });

  it('does not scan beyond 20 000 chars (ReDoS guard)', () => {
    // An email placed just past the 20 KB cap must be ignored entirely.
    const padding = 'x'.repeat(20_001);
    const result = extractRecipient(`${padding}hr@company.com`);
    expect(result.email).toBeUndefined();
  });

  it('ignores email addresses that appear after a very long gap from any name', () => {
    // The name is more than 150 chars away — window won't include it.
    // Pad with spaces only so the boundary before the email is unambiguous.
    const far = 'Jane Doe'.padEnd(200, ' ');
    const result = extractRecipient(`${far}jobs@company.com`);
    // email is still extracted, but name is too far back to be captured
    expect(result.email).toBe('jobs@company.com');
    expect(result.name).toBeUndefined();
  });
});
