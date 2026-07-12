/**
 * en/de parity for the interview-practice keys (mock Q&A + STAR feedback).
 * Uses the REAL @ajh/translations instance (not the identity mock the
 * component test uses) so a key present in only one locale fails here instead
 * of shipping the raw key string as UI copy. Mirrors AutopilotCard.i18n.test.ts.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;

const KEYS = [
  'applications.detail.interview.toggle.label',
  'applications.detail.interview.toggle.ask',
  'applications.detail.interview.toggle.practice',
  'applications.detail.interview.practice.hint',
  'applications.detail.interview.practice.generate',
  'applications.detail.interview.practice.regenerate',
  'applications.detail.interview.practice.generating',
  'applications.detail.interview.practice.empty',
  'applications.detail.interview.practice.emptyDesc',
  'applications.detail.interview.practice.answerPlaceholder',
  'applications.detail.interview.practice.getFeedback',
  'applications.detail.interview.practice.gettingFeedback',
  'applications.detail.interview.practice.type.behavioral',
  'applications.detail.interview.practice.type.roleSpecific',
  'applications.detail.interview.practice.type.technical',
  'applications.detail.interview.practice.feedback.strengths',
  'applications.detail.interview.practice.feedback.gaps',
  'applications.detail.interview.practice.feedback.starCompleteness',
  'applications.detail.interview.practice.feedback.situation',
  'applications.detail.interview.practice.feedback.task',
  'applications.detail.interview.practice.feedback.action',
  'applications.detail.interview.practice.feedback.result',
  'applications.detail.interview.practice.feedback.present',
  'applications.detail.interview.practice.feedback.missing',
  'applications.detail.interview.practice.feedback.rewrite',
] as const;

describe('interview-practice i18n — en/de parity', () => {
  it.each(LOCALES)('%s resolves every practice-mode key to real copy', (lng) => {
    const t = i18n.getFixedT(lng);
    for (const key of KEYS) {
      const out = t(key);
      expect(out, `${lng}:${key}`).not.toBe(key);
      expect(out.trim().length, `${lng}:${key}`).toBeGreaterThan(0);
    }
  });
});
