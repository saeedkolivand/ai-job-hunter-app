import { useCallback, useRef, useState } from 'react';

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

  /** Anchor to scroll+highlight after the next section renders. */
  const [pendingAnchor, setPendingAnchor] = useState<string | null>(null);

  const handleSectionChange = (v: SectionId) => {
    setActiveSection(v);
    scrollRef.current?.scrollTo({ top: 0, behavior: 'smooth' });
  };

  const handleResultSelect = (section: SectionId, anchor: string) => {
    if (section !== activeSection) {
      setActiveSection(section);
    }
    // Always set the anchor — SettingsContent fires the scroll after mount.
    setPendingAnchor(anchor);
  };

  const handleAnchorConsumed = useCallback(() => setPendingAnchor(null), []);

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
    <PageTransition className="h-full overflow-hidden">
      <div className="mx-auto flex h-full w-full max-w-6xl overflow-hidden 2xl:max-w-7xl">
        <SettingsSidebar
          navGroups={navGroups}
          activeSection={activeSection}
          onSectionChange={handleSectionChange}
          onResultSelect={handleResultSelect}
        />
        <SettingsContent
          activeSection={activeSection}
          current={current}
          localName={localName}
          setLocalName={setLocalName}
          setUserName={setUserName}
          userName={userName ?? ''}
          scrollRef={scrollRef}
          pendingAnchor={pendingAnchor}
          onAnchorConsumed={handleAnchorConsumed}
        />
      </div>
    </PageTransition>
  );
}
