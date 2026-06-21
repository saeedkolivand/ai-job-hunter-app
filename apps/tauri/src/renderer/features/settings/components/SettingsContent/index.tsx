import { AnimatePresence, motion } from 'motion/react';
import type { RefObject } from 'react';

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
  scrollRef: RefObject<HTMLDivElement | null>;
  setLocalName: (name: string) => void;
  setUserName: (name: string) => void;
  userName: string;
}

export function SettingsContent({
  activeSection,
  current,
  localName,
  scrollRef,
  setLocalName,
  setUserName,
  userName,
}: Props) {
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
            {activeSection === 'general' && (
              <GeneralSection
                localName={localName}
                setLocalName={setLocalName}
                setUserName={setUserName}
                userName={userName}
              />
            )}
            {activeSection === 'appearance' && <AppearanceCard />}
            {activeSection === 'contact' && <ContactProfileTab />}
            {activeSection === 'ai' && (
              <>
                <AISettingsTab />
                <OutputTonePreferences />
              </>
            )}
            {activeSection === 'job' && (
              <>
                <JobLocationPreferences />
                <TechStackPreferences />
                <AggregatorKeysSettings />
              </>
            )}
            {activeSection === 'resume' && <ResumePreferences />}
            {activeSection === 'accounts' && <AccountsSettingsTab />}
            {activeSection === 'privacy' && <PrivacySettingsTab />}
            {activeSection === 'performance' && <PerformancePreferences />}
            {activeSection === 'developer' && <DeveloperPreferences />}
            {activeSection === 'about' && <AboutTab />}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
