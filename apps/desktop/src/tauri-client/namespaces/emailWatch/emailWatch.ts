import { invoke } from '@tauri-apps/api/core';

import type { EmailWatchConnectRequest, EmailWatchStatus } from '@ajh/shared';

export const emailWatch = {
  status: () => invoke<EmailWatchStatus>('email_watch_status'),
  connect: (req: EmailWatchConnectRequest) =>
    invoke<EmailWatchStatus>('email_watch_connect', {
      address: req.address,
      appPassword: req.appPassword,
    }),
  disconnect: () => invoke<EmailWatchStatus>('email_watch_disconnect'),
  setEnabled: (enabled: boolean) =>
    invoke<EmailWatchStatus>('email_watch_set_enabled', { enabled }),
  checkNow: () => invoke<EmailWatchStatus>('email_watch_check_now'),
};
