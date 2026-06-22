import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, type RefObject, useEffect } from 'react';

import { IconBadge, transition, variants } from '@ajh/ui';

import { AboutTab } from '@/features/settings/components/about/AboutTab';
import { AccountsSettingsTab } from '@/features/settings/components/accounts/AccountsSettingsTab';
import { AISettingsTab } from '@/features/settings/components/ai-settings/AISettingsTab';
import { ContactProfileTab } from '@/features/settings/components/contact/ContactProfileTab';
import { GeneralSection } from '@/features/settings/components/general-section';
import { AppearanceCard } from '@/features/settings/components/general-section/AppearanceCard';
import { AggregatorKeysSettings } from '@/features/settings/components/preferences/AggregatorKeysSettings';
import { DeveloperPreferences } from '@/features/settings/components/preferences/DeveloperPreferences';
import { JobLocationPreferences } from '@/features/settings/components/preferences/JobLocationPreferences';
import { OutputTonePreferences } from '@/features/settings/components/preferences/OutputTonePreferences';
import { PerformancePreferences } from '@/features/settings/components/preferences/PerformancePreferences';
import { ResumePreferences } from '@/features/settings/components/preferences/ResumePreferences';
import { TechStackPreferences } from '@/features/settings/components/preferences/TechStackPreferences';
import { PrivacySettingsTab } from '@/features/settings/components/privacy/PrivacySettingsTab';
import type { NavItem, SectionId } from '@/features/settings/constants';

interface Props {
  activeSection: SectionId;
  current: NavItem;
  localName: string;
  /** Anchor to scroll+pulse after mount. Null when no pending anchor. */
  pendingAnchor: string | null;
  scrollRef: RefObject<HTMLDivElement | null>;
  setLocalName: (name: string) => void;
  setUserName: (name: string) => void;
  userName: string;
  /** Called after the anchor has been consumed so the parent can clear it. */
  onAnchorConsumed: () => void;
}

/** Duration (ms) the ring-pulse stays visible. */
const PULSE_DURATION = 1500;

/** Classes added during the pulse — must all be removed in cleanup. */
const PULSE_CLASSES = ['ring-2', 'ring-brand', 'rounded-xl', 'transition-[box-shadow]'] as const;

export function SettingsContent({
  activeSection,
  current,
  localName,
  pendingAnchor,
  scrollRef,
  setLocalName,
  setUserName,
  userName,
  onAnchorConsumed,
}: Props) {
  const sectionRegistry: Record<SectionId, () => ReactNode> = {
    general: () => (
      <GeneralSection
        localName={localName}
        setLocalName={setLocalName}
        setUserName={setUserName}
        userName={userName}
      />
    ),
    appearance: () => <AppearanceCard />,
    contact: () => <ContactProfileTab />,
    ai: () => (
      <>
        <div data-settings-anchor="ai-provider">
          <AISettingsTab />
        </div>
        <div data-settings-anchor="ai-tone">
          <OutputTonePreferences />
        </div>
      </>
    ),
    job: () => (
      <>
        <div data-settings-anchor="job-location">
          <JobLocationPreferences />
        </div>
        <div data-settings-anchor="job-techstack">
          <TechStackPreferences />
        </div>
        <div data-settings-anchor="job-aggregator">
          <AggregatorKeysSettings />
        </div>
      </>
    ),
    resume: () => (
      <div data-settings-anchor="resume-manage">
        <ResumePreferences />
      </div>
    ),
    accounts: () => <AccountsSettingsTab />,
    privacy: () => <PrivacySettingsTab />,
    performance: () => (
      <div data-settings-anchor="performance-mode">
        <PerformancePreferences />
      </div>
    ),
    developer: () => (
      <div data-settings-anchor="developer-tools">
        <DeveloperPreferences />
      </div>
    ),
    about: () => (
      <div data-settings-anchor="about-info">
        <AboutTab />
      </div>
    ),
  };
  /**
   * Scroll to + briefly ring-highlight an anchored element after the section
   * mounts. Keyed on both activeSection and pendingAnchor so it re-fires when
   * the section changes (the anchor element may not exist yet on the previous
   * render).
   */
  useEffect(() => {
    if (!pendingAnchor) return;

    let timer: ReturnType<typeof setTimeout> | undefined;
    let anchoredEl: HTMLElement | null = null;

    const removePulseClasses = () => {
      anchoredEl?.classList.remove(...PULSE_CLASSES);
    };

    // Use rAF so the section's DOM has finished painting.
    const rafId = requestAnimationFrame(() => {
      const el = scrollRef.current?.querySelector<HTMLElement>(
        `[data-settings-anchor="${pendingAnchor}"]`
      );
      if (!el) return;
      anchoredEl = el;

      const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

      if (reducedMotion) {
        // Skip animation; just scroll instantly and consume the anchor.
        el.scrollIntoView({ behavior: 'instant', block: 'nearest' });
        onAnchorConsumed();
      } else {
        el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        el.classList.add(...PULSE_CLASSES);

        timer = setTimeout(() => {
          removePulseClasses();
          onAnchorConsumed();
        }, PULSE_DURATION);
      }
    });

    return () => {
      cancelAnimationFrame(rafId);
      clearTimeout(timer);
      removePulseClasses();
    };
    // activeSection is intentionally included: the anchor DOM only exists after
    // the right section has rendered, so we re-run on section change too.
  }, [activeSection, pendingAnchor, scrollRef, onAnchorConsumed]);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Section header */}
      <div className="shrink-0 px-8 py-6">
        <div className="flex items-center gap-3">
          <IconBadge icon={current.icon} size="md" />
          <div>
            <div className="text-base font-semibold text-foreground/90">{current.label}</div>
            <div className="text-[11px] text-foreground/40">{current.description}</div>
          </div>
        </div>
      </div>

      {/* Scrollable content */}
      <div ref={scrollRef} className="@container flex-1 overflow-y-auto px-8 pb-6 pt-3">
        <AnimatePresence mode="wait">
          <motion.div
            key={activeSection}
            {...variants.fadeSlideUp}
            transition={transition.normal}
            className="max-w-2xl space-y-4"
          >
            {sectionRegistry[activeSection]()}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
