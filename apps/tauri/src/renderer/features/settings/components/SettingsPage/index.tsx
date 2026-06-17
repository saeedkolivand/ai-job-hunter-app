import { useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';

import { PageTransition } from '@/components/layout/PageTransition';
import { SettingsContent } from '@/features/settings/components/SettingsContent';
import { SettingsSidebar } from '@/features/settings/components/SettingsSidebar';
import {
  NAV_GROUPS,
  type NavGroup,
  type NavItem,
  type SectionId,
} from '@/features/settings/constants';
import { usePreferencesStore, useUserName } from '@/store/preferences-store';
import { useSessionStore } from '@/store/session-store';

export function SettingsPage() {
  const { t } = useTranslation();

  const { settings, setSettings } = useSessionStore();
  const activeSection = settings.activeSection;
  const setActiveSection = (v: SectionId) => setSettings({ activeSection: v });
  const scrollRef = useRef<HTMLDivElement>(null);
  const handleSectionChange = (v: SectionId) => {
    setActiveSection(v);
    scrollRef.current?.scrollTo({ top: 0, behavior: 'smooth' });
  };
  const userName = useUserName();
  const setUserName = usePreferencesStore((s) => s.setUserName);
  const [localName, setLocalName] = useState(userName || '');

  const navGroups = NAV_GROUPS.map((group) => ({
    ...group,
    label: t(group.label),
    items: group.items.map((item) => ({
      ...item,
      label: t(item.label),
      description: t(item.description),
    })),
  }));

  const allItems = navGroups.flatMap((g: NavGroup) => g.items);
  const current = allItems.find((i: NavItem) => i.id === activeSection) as NavItem;

  return (
    <PageTransition className="flex h-full overflow-hidden">
      <SettingsSidebar
        navGroups={navGroups}
        activeSection={activeSection}
        onSectionChange={handleSectionChange}
      />
      <SettingsContent
        activeSection={activeSection}
        current={current}
        localName={localName}
        setLocalName={setLocalName}
        setUserName={setUserName}
        userName={userName ?? ''}
        scrollRef={scrollRef}
      />
    </PageTransition>
  );
}
