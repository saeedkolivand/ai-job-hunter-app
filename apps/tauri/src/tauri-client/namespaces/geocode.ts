import { invoke } from '@tauri-apps/api/core';

export const geocode = {
  suggest: (query: string) =>
    invoke('geocode_suggest', { query }) as Promise<Array<{ display: string }>>,
};
