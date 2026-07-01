import { invoke } from '@tauri-apps/api/core';

export const data = {
  export: () => invoke('data_export'),
  import: () => invoke('data_import'),
};
