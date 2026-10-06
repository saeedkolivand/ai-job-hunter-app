import { Bot, Briefcase, Link as LinkIcon, Settings, Wifi } from 'lucide-react';

import type { Section, Translate } from './types';

/** Troubleshooting sections: scraping, AI features, accounts, general and connectivity problems. */
export const troubleshootingSections = (t: Translate): Section[] => [
  {
    icon: Briefcase,
    id: 'jobScraping',
    label: t('support.faq.jobScraping'),
    color: 'text-purple-400',
    glow: 'rgba(168,85,247,0.15)',
    problems: [
      {
        id: 'linkedinNoResults',
        q: t('support.faq.jobScrapingQuestions.linkedinNoResults.q'),
        a: t('support.faq.jobScrapingQuestions.linkedinNoResults.a'),
      },
      {
        id: 'scrapingZeroJobs',
        q: t('support.faq.jobScrapingQuestions.scrapingZeroJobs.q'),
        a: t('support.faq.jobScrapingQuestions.scrapingZeroJobs.a'),
      },
      {
        id: 'jobsDisappeared',
        q: t('support.faq.jobScrapingQuestions.jobsDisappeared.q'),
        a: t('support.faq.jobScrapingQuestions.jobsDisappeared.a'),
      },
      {
        id: 'clearButtonRemoved',
        q: t('support.faq.jobScrapingQuestions.clearButtonRemoved.q'),
        a: t('support.faq.jobScrapingQuestions.clearButtonRemoved.a'),
      },
    ],
  },
  {
    icon: Bot,
    id: 'aiFeatures',
    label: t('support.faq.aiFeatures'),
    color: 'text-blue-400',
    glow: 'rgba(59,130,246,0.15)',
    problems: [
      {
        id: 'aiDoesNothing',
        q: t('support.faq.aiFeaturesQuestions.aiDoesNothing.q'),
        a: t('support.faq.aiFeaturesQuestions.aiDoesNothing.a'),
      },
      {
        id: 'outputToneWrong',
        q: t('support.faq.aiFeaturesQuestions.outputToneWrong.q'),
        a: t('support.faq.aiFeaturesQuestions.outputToneWrong.a'),
      },
      {
        id: 'noRecommendations',
        q: t('support.faq.aiFeaturesQuestions.noRecommendations.q'),
        a: t('support.faq.aiFeaturesQuestions.noRecommendations.a'),
      },
    ],
  },
  {
    icon: LinkIcon,
    id: 'accountsSessions',
    label: t('support.faq.accountsSessions'),
    color: 'text-emerald-400',
    glow: 'rgba(16,185,129,0.15)',
    problems: [
      {
        id: 'browserWindowNotOpen',
        q: t('support.faq.accountsSessionsQuestions.browserWindowNotOpen.q'),
        a: t('support.faq.accountsSessionsQuestions.browserWindowNotOpen.a'),
      },
      {
        id: 'linkedinGuestMode',
        q: t('support.faq.accountsSessionsQuestions.linkedinGuestMode.q'),
        a: t('support.faq.accountsSessionsQuestions.linkedinGuestMode.a'),
      },
    ],
  },
  {
    icon: Settings,
    id: 'general',
    label: t('support.faq.general'),
    color: 'text-amber-400',
    glow: 'rgba(245,158,11,0.15)',
    problems: [
      {
        id: 'interactionHistoryGone',
        q: t('support.faq.generalQuestions.interactionHistoryGone.q'),
        a: t('support.faq.generalQuestions.interactionHistoryGone.a'),
      },
      {
        id: 'appSlow',
        q: t('support.faq.generalQuestions.appSlow.q'),
        a: t('support.faq.generalQuestions.appSlow.a'),
      },
      {
        id: 'resetEverything',
        q: t('support.faq.generalQuestions.resetEverything.q'),
        a: t('support.faq.generalQuestions.resetEverything.a'),
      },
      {
        id: 'changeLanguage',
        q: t('support.faq.generalQuestions.changeLanguage.q'),
        a: t('support.faq.generalQuestions.changeLanguage.a'),
      },
      {
        id: 'checkForUpdates',
        q: t('support.faq.generalQuestions.checkForUpdates.q'),
        a: t('support.faq.generalQuestions.checkForUpdates.a'),
      },
    ],
  },
  {
    icon: Wifi,
    id: 'connectivity',
    label: t('support.faq.connectivity'),
    color: 'text-red-400',
    glow: 'rgba(239,68,68,0.15)',
    problems: [
      {
        id: 'networkError',
        q: t('support.faq.connectivityQuestions.networkError.q'),
        a: t('support.faq.connectivityQuestions.networkError.a'),
      },
      {
        id: 'captchaAppears',
        q: t('support.faq.connectivityQuestions.captchaAppears.q'),
        a: t('support.faq.connectivityQuestions.captchaAppears.a'),
      },
    ],
  },
];
