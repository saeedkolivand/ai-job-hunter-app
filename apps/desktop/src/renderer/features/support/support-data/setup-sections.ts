import { Cpu, ShieldCheck, Terminal } from 'lucide-react';

import type { Section, Translate } from './types';

/** How-to sections about configuring AI providers, the agent CLI and privacy. */
export const setupSections = (t: Translate): Section[] => [
  {
    icon: Cpu,
    id: 'aiSetup',
    label: t('support.faq.aiSetup'),
    color: 'text-fuchsia-400',
    glow: 'rgba(217,70,239,0.15)',
    problems: [
      {
        id: 'chooseProvider',
        q: t('support.faq.aiSetupQuestions.chooseProvider.q'),
        a: t('support.faq.aiSetupQuestions.chooseProvider.a'),
      },
      {
        id: 'perStageModels',
        q: t('support.faq.aiSetupQuestions.perStageModels.q'),
        a: t('support.faq.aiSetupQuestions.perStageModels.a'),
      },
      {
        id: 'spend',
        q: t('support.faq.aiSetupQuestions.spend.q'),
        a: t('support.faq.aiSetupQuestions.spend.a'),
      },
    ],
  },
  {
    icon: Terminal,
    id: 'agentCli',
    label: t('support.faq.agentCli'),
    color: 'text-slate-400',
    glow: 'rgba(148,163,184,0.15)',
    problems: [
      {
        id: 'headlessMode',
        q: t('support.faq.agentCliQuestions.headlessMode.q'),
        a: t('support.faq.agentCliQuestions.headlessMode.a'),
      },
    ],
  },
  {
    icon: ShieldCheck,
    id: 'privacy',
    label: t('support.faq.privacy'),
    color: 'text-rose-400',
    glow: 'rgba(244,63,94,0.15)',
    problems: [
      {
        id: 'exportImport',
        q: t('support.faq.privacyQuestions.exportImport.q'),
        a: t('support.faq.privacyQuestions.exportImport.a'),
      },
      {
        id: 'whatLeaves',
        q: t('support.faq.privacyQuestions.whatLeaves.q'),
        a: t('support.faq.privacyQuestions.whatLeaves.a'),
      },
    ],
  },
];
