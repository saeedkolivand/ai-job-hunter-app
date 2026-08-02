import { invoke } from '@tauri-apps/api/core';

import type { CrashReportingSettings } from '@ajh/shared';

export const privacy = {
  signOutAll: () => invoke('privacy_sign_out_all'),
  clearInteractions: () => invoke('privacy_clear_interactions'),
  resetApp: () => invoke('privacy_reset_app'),
  getCrashReporting: () => invoke<CrashReportingSettings>('privacy_get_crash_reporting'),
  setCrashReporting: (settings: CrashReportingSettings) =>
    invoke<CrashReportingSettings>('privacy_set_crash_reporting', { settings }),
};
