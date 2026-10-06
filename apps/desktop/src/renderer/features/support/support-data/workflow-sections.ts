import {
  ClipboardList,
  FileText,
  Puzzle,
  Radar,
  Rocket,
  Search,
  Sparkles,
  Target,
} from 'lucide-react';

import type { Section, Translate } from './types';

/** How-to sections that follow the job-search workflow, from first launch to the browser extension. */
export const workflowSections = (t: Translate): Section[] => [
  {
    icon: Rocket,
    id: 'gettingStarted',
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
      {
        id: 'githubProjects',
        q: t('support.faq.gettingStartedQuestions.githubProjects.q'),
        a: t('support.faq.gettingStartedQuestions.githubProjects.a'),
      },
    ],
  },
  {
    icon: Search,
    id: 'findingJobs',
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
    id: 'matchScore',
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
    id: 'documents',
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
      {
        id: 'importFromLinkedin',
        q: t('support.faq.documentsQuestions.importFromLinkedin.q'),
        a: t('support.faq.documentsQuestions.importFromLinkedin.a'),
      },
      {
        id: 'contactHeader',
        q: t('support.faq.documentsQuestions.contactHeader.q'),
        a: t('support.faq.documentsQuestions.contactHeader.a'),
      },
    ],
  },
  {
    icon: Sparkles,
    id: 'aiGenerate',
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
    id: 'applications',
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
      {
        id: 'applicantDetails',
        q: t('support.faq.applicationsQuestions.applicantDetails.q'),
        a: t('support.faq.applicationsQuestions.applicantDetails.a'),
      },
    ],
  },
  {
    icon: Radar,
    id: 'autopilot',
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
    id: 'extension',
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
];
