import { useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import { PageTransition } from '@/components/layout/PageTransition';
import { useTranslation } from '@/lib/i18n';
import { usePreferencesStore, useUserName } from '@/store/preferences-store';
import { useSessionStore } from '@/store/session-store';

import { SettingsContent } from './components/SettingsContent';
import { SettingsSidebar } from './components/SettingsSidebar';
import { NAV_GROUPS, type NavGroup, type NavItem, type SectionId } from './constants';

export const Route = createFileRoute('/settings/')({ component: SettingsPage });

function SettingsPage() {
  const { t } = useTranslation();

  const { settings, setSettings } = useSessionStore();
  const activeSection = settings.activeSection as SectionId;
  const setActiveSection = (v: SectionId) => setSettings({ activeSection: v });
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
        onSectionChange={setActiveSection}
      />
      <SettingsContent
        activeSection={activeSection}
        current={current}
        localName={localName}
        setLocalName={setLocalName}
        setUserName={setUserName}
        userName={userName ?? ''}
      />
    </PageTransition>
  );
}
