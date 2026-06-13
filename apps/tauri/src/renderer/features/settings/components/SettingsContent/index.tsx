import { AnimatePresence, motion } from 'motion/react';

import { IconBadge, transition, variants } from '@ajh/ui';

import { AccountsSettingsTab } from '@/features/settings/components/accounts/AccountsSettingsTab';
import { AISettingsTab } from '@/features/settings/components/ai-settings/AISettingsTab';
import { ContactProfileTab } from '@/features/settings/components/contact/ContactProfileTab';
import { GeneralSection } from '@/features/settings/components/general-section';
import { AppearanceCard } from '@/features/settings/components/general-section/AppearanceCard';
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
  setLocalName: (name: string) => void;
  setUserName: (name: string) => void;
  userName: string;
}

export function SettingsContent({
  activeSection,
  current,
  localName,
  setLocalName,
  setUserName,
  userName,
}: Props) {
  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Section header */}
      <div className="shrink-0 border-white/[0.05] px-8 py-6">
        <div className="flex items-center gap-3">
          <IconBadge icon={current.icon} size="md" />
          <div>
            <div className="text-base font-semibold text-foreground/90">{current.label}</div>
            <div className="text-[11px] text-foreground/40">{current.description}</div>
          </div>
        </div>
      </div>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto px-8 py-6">
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
              </>
            )}
            {activeSection === 'resume' && <ResumePreferences />}
            {activeSection === 'accounts' && <AccountsSettingsTab />}
            {activeSection === 'privacy' && <PrivacySettingsTab />}
            {activeSection === 'performance' && <PerformancePreferences />}
            {activeSection === 'developer' && <DeveloperPreferences />}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
