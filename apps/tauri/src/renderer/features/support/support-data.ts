import { Bot, Briefcase, Link as LinkIcon, Settings, Wifi } from 'lucide-react';

export interface Problem {
  q: string;
  a: string;
}

export interface Section {
  icon: React.ElementType;
  label: string;
  color: string;
  glow: string;
  problems: Problem[];
}

export function getSupportSections(t: (key: string) => string): Section[] {
  return [
    {
      icon: Briefcase,
      label: t('support.faq.jobScraping'),
      color: 'text-purple-400',
      glow: 'rgba(168,85,247,0.15)',
      problems: [
        {
          q: t('support.faq.jobScrapingQuestions.linkedinNoResults.q'),
          a: t('support.faq.jobScrapingQuestions.linkedinNoResults.a'),
        },
        {
          q: t('support.faq.jobScrapingQuestions.scrapingZeroJobs.q'),
          a: t('support.faq.jobScrapingQuestions.scrapingZeroJobs.a'),
        },
        {
          q: t('support.faq.jobScrapingQuestions.jobsDisappeared.q'),
          a: t('support.faq.jobScrapingQuestions.jobsDisappeared.a'),
        },
        {
          q: t('support.faq.jobScrapingQuestions.clearButtonRemoved.q'),
          a: t('support.faq.jobScrapingQuestions.clearButtonRemoved.a'),
        },
      ],
    },
    {
      icon: Bot,
      label: t('support.faq.aiFeatures'),
      color: 'text-blue-400',
      glow: 'rgba(59,130,246,0.15)',
      problems: [
        {
          q: t('support.faq.aiFeaturesQuestions.aiDoesNothing.q'),
          a: t('support.faq.aiFeaturesQuestions.aiDoesNothing.a'),
        },
        {
          q: t('support.faq.aiFeaturesQuestions.outputToneWrong.q'),
          a: t('support.faq.aiFeaturesQuestions.outputToneWrong.a'),
        },
        {
          q: t('support.faq.aiFeaturesQuestions.noRecommendations.q'),
          a: t('support.faq.aiFeaturesQuestions.noRecommendations.a'),
        },
      ],
    },
    {
      icon: LinkIcon,
      label: t('support.faq.accountsSessions'),
      color: 'text-emerald-400',
      glow: 'rgba(16,185,129,0.15)',
      problems: [
        {
          q: t('support.faq.accountsSessionsQuestions.browserWindowNotOpen.q'),
          a: t('support.faq.accountsSessionsQuestions.browserWindowNotOpen.a'),
        },
        {
          q: t('support.faq.accountsSessionsQuestions.linkedinGuestMode.q'),
          a: t('support.faq.accountsSessionsQuestions.linkedinGuestMode.a'),
        },
      ],
    },
    {
      icon: Settings,
      label: t('support.faq.general'),
      color: 'text-amber-400',
      glow: 'rgba(245,158,11,0.15)',
      problems: [
        {
          q: t('support.faq.generalQuestions.interactionHistoryGone.q'),
          a: t('support.faq.generalQuestions.interactionHistoryGone.a'),
        },
        {
          q: t('support.faq.generalQuestions.appSlow.q'),
          a: t('support.faq.generalQuestions.appSlow.a'),
        },
        {
          q: t('support.faq.generalQuestions.languageNotChanged.q'),
          a: t('support.faq.generalQuestions.languageNotChanged.a'),
        },
        {
          q: t('support.faq.generalQuestions.resetEverything.q'),
          a: t('support.faq.generalQuestions.resetEverything.a'),
        },
      ],
    },
    {
      icon: Wifi,
      label: t('support.faq.connectivity'),
      color: 'text-red-400',
      glow: 'rgba(239,68,68,0.15)',
      problems: [
        {
          q: t('support.faq.connectivityQuestions.networkError.q'),
          a: t('support.faq.connectivityQuestions.networkError.a'),
        },
        {
          q: t('support.faq.connectivityQuestions.captchaAppears.q'),
          a: t('support.faq.connectivityQuestions.captchaAppears.a'),
        },
      ],
    },
  ];
}
