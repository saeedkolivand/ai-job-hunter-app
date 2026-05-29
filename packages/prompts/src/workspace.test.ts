import { describe, expect, it } from 'vitest';

import { buildWorkspaceSystemPrompt } from './workspace';

describe('buildWorkspaceSystemPrompt', () => {
  it('instructs the assistant to reply in the requested locale', () => {
    const prompt = buildWorkspaceSystemPrompt({ locale: 'de' });
    expect(prompt).toContain('Always reply in de');
  });

  it('uses the provided locale verbatim', () => {
    expect(buildWorkspaceSystemPrompt({ locale: 'fr' })).toContain('Always reply in fr');
    expect(buildWorkspaceSystemPrompt({ locale: 'es' })).toContain('Always reply in es');
  });

  it('prompts the user to upload a resume when none is provided', () => {
    const prompt = buildWorkspaceSystemPrompt({ locale: 'en' });
    expect(prompt).toContain('No resume uploaded yet');
  });

  it('embeds the resume (truncated) when provided', () => {
    const resumeText = 'R'.repeat(5000);
    const prompt = buildWorkspaceSystemPrompt({ locale: 'en', resumeText });
    expect(prompt).toContain("USER'S RESUME");
    // Resume body is capped at 4000 characters.
    expect(prompt).toContain('R'.repeat(4000));
    expect(prompt).not.toContain('R'.repeat(4001));
  });

  it('always documents the core app features', () => {
    const prompt = buildWorkspaceSystemPrompt({ locale: 'en' });
    expect(prompt).toContain('RESUME ANALYZER');
    expect(prompt).toContain('AUTOPILOT');
    expect(prompt).toContain('OLLAMA SETUP');
  });
});
