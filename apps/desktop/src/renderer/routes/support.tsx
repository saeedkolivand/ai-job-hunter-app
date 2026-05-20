import { Bot, Briefcase, ChevronDown, Link as LinkIcon, Settings, Wifi } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import { Button } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';

export const Route = createFileRoute('/support')({ component: SupportPage });

interface Problem {
  q: string;
  a: string;
}

interface Section {
  icon: React.ElementType;
  label: string;
  color: string;
  glow: string;
  problems: Problem[];
}

function SupportPage() {
  const { t } = useTranslation();

  const SECTIONS: Section[] = [
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
          q: t('support.faq.jobScrapingQuestions.indeedWrongCountry.q'),
          a: t('support.faq.jobScrapingQuestions.indeedWrongCountry.a'),
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

  return (
    <PageTransition className="h-full overflow-y-auto px-10 py-10">
      <div className="mx-auto max-w-3xl">
        <PageHeader
          title={t('support.faq.title')}
          subtitle={t('support.faq.subtitle')}
          badge={t('support.faq.badge')}
        />

        <div className="mt-2 space-y-8">
          {SECTIONS.map((section) => {
            const Icon = section.icon;
            return (
              <div key={section.label}>
                {/* Section header */}
                <div className="mb-3 flex items-center gap-2.5">
                  <div
                    className="flex h-7 w-7 items-center justify-center rounded-lg"
                    style={{ background: section.glow }}
                  >
                    <Icon size={14} className={section.color} />
                  </div>
                  <span className="text-xs font-semibold uppercase tracking-[0.18em] text-foreground/40">
                    {section.label}
                  </span>
                </div>

                {/* Accordion */}
                <div className="space-y-2">
                  {section.problems.map((p, i) => (
                    <AccordionItem key={i} problem={p} />
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </PageTransition>
  );
}

function AccordionItem({
  problem,
  defaultOpen = false,
}: {
  problem: Problem;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div
      className={cn(
        'overflow-hidden rounded-xl border transition-colors duration-150',
        open ? 'border-white/10 bg-white/[0.04]' : 'border-white/[0.05] bg-white/[0.02]'
      )}
    >
      <Button
        variant="ghost"
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left hover:bg-transparent"
      >
        <span
          className={cn(
            'text-sm font-medium transition-colors',
            open ? 'text-foreground/90' : 'text-foreground/65'
          )}
        >
          {problem.q}
        </span>
        <ChevronDown
          size={15}
          className={cn(
            'shrink-0 text-foreground/30 transition-transform duration-200',
            open && 'rotate-180'
          )}
        />
      </Button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={transition.relaxed}
          >
            <div className="border-t border-white/[0.05] px-5 py-4 text-sm leading-relaxed text-foreground/60">
              <div dangerouslySetInnerHTML={{ __html: problem.a }} />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
