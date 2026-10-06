import { describe, expect, it, vi } from 'vitest';

import { usePreferencesStore } from '@/store/preferences-store';

import {
  generateApplicationAnswer,
  generateApplicationEmail,
  generateCoverLetter,
  generateResume,
} from './generation';
import { installGenerationHooks, register, streamThrough } from './test-support';

installGenerationHooks();

describe('output tone wiring (Settings → Output Tone)', () => {
  // The system prompt is always messages[0] (see streamGenerate in generation.ts).
  const systemOf = (client: ReturnType<typeof register>) => {
    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    const messages = (call?.[0] as { messages: { role: string; content: string }[] }).messages;
    return messages[0]?.content ?? '';
  };

  const META = {
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    candidateName: 'X',
    jobTitle: 'Y',
    companyName: 'Z',
    targetLanguage: 'en',
    topRequirements: [],
  };

  it('threads the store outputTone into the resume system prompt', async () => {
    usePreferencesStore.setState({ outputTone: 'casual' });
    const client = register();
    await streamThrough(
      generateResume('My resume', 'Job ad', META, 'ats', 'llama3', vi.fn()),
      'RESUME CONTENT'
    );
    expect(systemOf(client)).toMatch(/TONE: conversational and casual/);
  });

  it('threads the store outputTone into the cover-letter system prompt', async () => {
    usePreferencesStore.setState({ outputTone: 'formal' });
    const client = register();
    await streamThrough(
      generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'llama3', vi.fn()),
      'Dear Hiring Team.'
    );
    expect(systemOf(client)).toMatch(/TONE: formal and precise/);
  });

  it('threads the store outputTone into the application-answer system prompt', async () => {
    usePreferencesStore.setState({ outputTone: 'creative' });
    const client = register();
    await streamThrough(
      generateApplicationAnswer({
        question: 'Why do you want to work here?',
        resume: 'My resume',
        jobAd: 'Backend role at Acme',
        meta: META,
        model: 'llama3',
      }),
      'Because I love building things.'
    );
    expect(systemOf(client)).toMatch(/TONE: a more narrative, distinctive voice/);
  });

  // The application-email path resolves its own market + tone (mirroring
  // generateCoverLetter); each is asserted alone so a regression in one is never
  // reported as a failure of the other.
  type EmailMeta = Parameters<typeof generateApplicationEmail>[0]['meta'];
  const runEmail = async (client: ReturnType<typeof register>, meta: EmailMeta) => {
    await streamThrough(
      generateApplicationEmail({
        resume: 'My resume',
        jobAd: 'Backend role in Berlin',
        meta,
        model: 'llama3',
      }),
      'Subject: Application\n\nGreeting.'
    );
    return systemOf(client);
  };

  it('threads the store outputTone into the application-email system prompt', async () => {
    usePreferencesStore.setState({ outputTone: 'formal' });
    const client = register();
    expect(await runEmail(client, META)).toMatch(/TONE: formal and precise/);
  });

  it('threads the market resolved from meta.jobCountry into the application-email prompt', async () => {
    const client = register();
    // English email for a German job: only the market resolved from `jobCountry`
    // can put a German salutation in the prompt — drop the `market` argument and
    // this falls back to the international "Dear Hiring Manager,".
    const system = await runEmail(client, { ...META, jobCountry: 'DE' });
    expect(system).toContain('Sehr geehrte Damen und Herren,');
    expect(system).not.toContain('Dear Hiring Manager,');
  });

  it('falls back to the target-language market when the job country is unknown', async () => {
    const client = register();
    // No jobCountry (the ApplyByEmail tab never sets one): resolveMarket falls
    // through to the letter language, so a German email still gets DACH etiquette.
    const system = await runEmail(client, { ...META, targetLanguage: 'de' });
    expect(system).toContain('Sehr geehrte Damen und Herren,');
    expect(system).not.toContain('Dear Hiring Manager,');
  });

  it('resolves to the professional tone directive by default (outputTone: professional)', async () => {
    const client = register();
    await streamThrough(
      generateResume('My resume', 'Job ad', META, 'ats', 'llama3', vi.fn()),
      'RESUME CONTENT'
    );
    expect(systemOf(client)).toMatch(/TONE: polished, warm, and professional/);
  });
});
