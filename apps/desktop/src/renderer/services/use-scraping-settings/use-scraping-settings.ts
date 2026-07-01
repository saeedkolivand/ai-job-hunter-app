/**
 * React Query hooks for non-secret scraping settings stored in the Tauri
 * plugin-store JSON file (`scraping-settings.json`). These settings are read
 * by the Rust aggregator at search time; the renderer owns writing them.
 *
 * Ports & Adapters: components import from `@/services`, not directly from
 * `@tauri-apps/plugin-store`.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Store } from '@tauri-apps/plugin-store';

import { SCRAPING_SETTINGS_FILE, SCRAPING_SETTINGS_KEYS } from '@ajh/shared';

import { keys, QUERY_TIMES } from '../query-client';

export interface ScrapingSettings {
  apifyLinkedinEnabled: boolean;
  apifyLinkedinActorId: string | undefined;
}

async function readScrapingSettings(): Promise<ScrapingSettings> {
  try {
    const store = await Store.load(SCRAPING_SETTINGS_FILE);
    const enabled = await store.get<boolean>(SCRAPING_SETTINGS_KEYS.apifyLinkedinEnabled);
    const actorId = await store.get<string>(SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId);
    return {
      apifyLinkedinEnabled: enabled === true,
      apifyLinkedinActorId: actorId?.trim() || undefined,
    };
  } catch {
    // Absent file or FS error → safe defaults (toggle off, no override).
    return { apifyLinkedinEnabled: false, apifyLinkedinActorId: undefined };
  }
}

async function writeScrapingSettings(patch: Partial<ScrapingSettings>): Promise<void> {
  const store = await Store.load(SCRAPING_SETTINGS_FILE);
  if (patch.apifyLinkedinEnabled !== undefined) {
    await store.set(SCRAPING_SETTINGS_KEYS.apifyLinkedinEnabled, patch.apifyLinkedinEnabled);
  }
  if (SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId in patch) {
    const val = patch.apifyLinkedinActorId?.trim();
    if (val) {
      await store.set(SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId, val);
    } else {
      await store.delete(SCRAPING_SETTINGS_KEYS.apifyLinkedinActorId);
    }
  }
  await store.save();
}

export const useScrapingSettings = () =>
  useQuery({
    queryKey: keys.scrapingSettings.all,
    queryFn: readScrapingSettings,
    staleTime: QUERY_TIMES.INFINITE,
  });

export const useUpdateScrapingSettings = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: writeScrapingSettings,
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.scrapingSettings.all }),
  });
};
