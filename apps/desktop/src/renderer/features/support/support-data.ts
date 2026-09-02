import {
  Bot,
  Briefcase,
  ClipboardList,
  Cpu,
  FileText,
  Link as LinkIcon,
  Puzzle,
  Radar,
  Rocket,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Target,
  Terminal,
  Wifi,
} from 'lucide-react';

interface Problem {
  /** Stable list key — the leaf of this entry's translation key. */
  id: string;
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

/**
 * The help corpus (ADR-041): how-to sections first, troubleshooting after.
 *
 * Every `t()` call is written out with a literal key so `i18next-cli` can see
 * it — a computed key would be invisible to the extractor and to the en/de
 * parity gate that rides on it.
 */
export function getSupportSections(t: (key: string) => string): Section[] {
  return [
    {
      icon: Rocket,
      label: t('support.faq.gettingStarted'),
      color: 'text-sky-400',
      glow: 'rgba(56,189,248,0.15)',
      problems: [
        {
          id: 'firstSteps',
          q: t('support.faq.gettingStartedQuestions.firstSteps.q'),
          a: t('support.faq.gettingStartedQuestions.firstSteps.a'),
        },
        {
          id: 'noResumeYet',
          q: t('support.faq.gettingStartedQuestions.noResumeYet.q'),
          a: t('support.faq.gettingStartedQuestions.noResumeYet.a'),
        },
        {
          id: 'replayWizard',
          q: t('support.faq.gettingStartedQuestions.replayWizard.q'),
          a: t('support.faq.gettingStartedQuestions.replayWizard.a'),
        },
      ],
    },
    {
      icon: Search,
      label: t('support.faq.findingJobs'),
      color: 'text-violet-400',
      glow: 'rgba(139,92,246,0.15)',
      problems: [
        {
          id: 'whichBoards',
          q: t('support.faq.findingJobsQuestions.whichBoards.q'),
          a: t('support.faq.findingJobsQuestions.whichBoards.a'),
        },
        {
          id: 'saveAJob',
          q: t('support.faq.findingJobsQuestions.saveAJob.q'),
          a: t('support.faq.findingJobsQuestions.saveAJob.a'),
        },
        {
          id: 'searchBox',
          q: t('support.faq.findingJobsQuestions.searchBox.q'),
          a: t('support.faq.findingJobsQuestions.searchBox.a'),
        },
        {
          id: 'narrowList',
          q: t('support.faq.findingJobsQuestions.narrowList.q'),
          a: t('support.faq.findingJobsQuestions.narrowList.a'),
        },
        {
          id: 'duplicates',
          q: t('support.faq.findingJobsQuestions.duplicates.q'),
          a: t('support.faq.findingJobsQuestions.duplicates.a'),
        },
      ],
    },
    {
      icon: Target,
      label: t('support.faq.matchScore'),
      color: 'text-teal-400',
      glow: 'rgba(45,212,191,0.15)',
      problems: [
        {
          id: 'whatIsScore',
          q: t('support.faq.matchScoreQuestions.whatIsScore.q'),
          a: t('support.faq.matchScoreQuestions.whatIsScore.a'),
        },
        {
          id: 'coverageVsMatch',
          q: t('support.faq.matchScoreQuestions.coverageVsMatch.q'),
          a: t('support.faq.matchScoreQuestions.coverageVsMatch.a'),
        },
        {
          id: 'saveResumeToScore',
          q: t('support.faq.matchScoreQuestions.saveResumeToScore.q'),
          a: t('support.faq.matchScoreQuestions.saveResumeToScore.a'),
        },
        {
          id: 'bestMatches',
          q: t('support.faq.matchScoreQuestions.bestMatches.q'),
          a: t('support.faq.matchScoreQuestions.bestMatches.a'),
        },
      ],
    },
    {
      icon: FileText,
      label: t('support.faq.documents'),
      color: 'text-indigo-400',
      glow: 'rgba(129,140,248,0.15)',
      problems: [
        {
          id: 'importFormats',
          q: t('support.faq.documentsQuestions.importFormats.q'),
          a: t('support.faq.documentsQuestions.importFormats.a'),
        },
        {
          id: 'indexed',
          q: t('support.faq.documentsQuestions.indexed.q'),
          a: t('support.faq.documentsQuestions.indexed.a'),
        },
        {
          id: 'multipleResumes',
          q: t('support.faq.documentsQuestions.multipleResumes.q'),
          a: t('support.faq.documentsQuestions.multipleResumes.a'),
        },
      ],
    },
    {
      icon: Sparkles,
      label: t('support.faq.aiGenerate'),
      color: 'text-blue-400',
      glow: 'rgba(59,130,246,0.15)',
      problems: [
        {
          id: 'tailorRun',
          q: t('support.faq.aiGenerateQuestions.tailorRun.q'),
          a: t('support.faq.aiGenerateQuestions.tailorRun.a'),
        },
        {
          id: 'needsReview',
          q: t('support.faq.aiGenerateQuestions.needsReview.q'),
          a: t('support.faq.aiGenerateQuestions.needsReview.a'),
        },
        {
          id: 'coverOnly',
          q: t('support.faq.aiGenerateQuestions.coverOnly.q'),
          a: t('support.faq.aiGenerateQuestions.coverOnly.a'),
        },
        {
          id: 'applicationAnswers',
          q: t('support.faq.aiGenerateQuestions.applicationAnswers.q'),
          a: t('support.faq.aiGenerateQuestions.applicationAnswers.a'),
        },
        {
          id: 'humanize',
          q: t('support.faq.aiGenerateQuestions.humanize.q'),
          a: t('support.faq.aiGenerateQuestions.humanize.a'),
        },
        {
          id: 'exportDoc',
          q: t('support.faq.aiGenerateQuestions.exportDoc.q'),
          a: t('support.faq.aiGenerateQuestions.exportDoc.a'),
        },
        {
          id: 'whereStored',
          q: t('support.faq.aiGenerateQuestions.whereStored.q'),
          a: t('support.faq.aiGenerateQuestions.whereStored.a'),
        },
      ],
    },
    {
      icon: ClipboardList,
      label: t('support.faq.applications'),
      color: 'text-emerald-400',
      glow: 'rgba(16,185,129,0.15)',
      problems: [
        {
          id: 'trackJob',
          q: t('support.faq.applicationsQuestions.trackJob.q'),
          a: t('support.faq.applicationsQuestions.trackJob.a'),
        },
        {
          id: 'remindersAndNotes',
          q: t('support.faq.applicationsQuestions.remindersAndNotes.q'),
          a: t('support.faq.applicationsQuestions.remindersAndNotes.a'),
        },
        {
          id: 'interviewPrep',
          q: t('support.faq.applicationsQuestions.interviewPrep.q'),
          a: t('support.faq.applicationsQuestions.interviewPrep.a'),
        },
        {
          id: 'emailTracking',
          q: t('support.faq.applicationsQuestions.emailTracking.q'),
          a: t('support.faq.applicationsQuestions.emailTracking.a'),
        },
        {
          id: 'referral',
          q: t('support.faq.applicationsQuestions.referral.q'),
          a: t('support.faq.applicationsQuestions.referral.a'),
        },
      ],
    },
    {
      icon: Radar,
      label: t('support.faq.autopilot'),
      color: 'text-cyan-400',
      glow: 'rgba(34,211,238,0.15)',
      problems: [
        {
          id: 'whatIsAutopilot',
          q: t('support.faq.autopilotQuestions.whatIsAutopilot.q'),
          a: t('support.faq.autopilotQuestions.whatIsAutopilot.a'),
        },
        {
          id: 'setUpAutopilot',
          q: t('support.faq.autopilotQuestions.setUpAutopilot.q'),
          a: t('support.faq.autopilotQuestions.setUpAutopilot.a'),
        },
      ],
    },
    {
      icon: Puzzle,
      label: t('support.faq.extension'),
      color: 'text-orange-400',
      glow: 'rgba(249,115,22,0.15)',
      problems: [
        {
          id: 'pairExtension',
          q: t('support.faq.extensionQuestions.pairExtension.q'),
          a: t('support.faq.extensionQuestions.pairExtension.a'),
        },
        {
          id: 'extensionActions',
          q: t('support.faq.extensionQuestions.extensionActions.q'),
          a: t('support.faq.extensionQuestions.extensionActions.a'),
        },
      ],
    },
    {
      icon: Cpu,
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
    {
      icon: Briefcase,
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
      ],
    },
    {
      icon: Wifi,
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
}
