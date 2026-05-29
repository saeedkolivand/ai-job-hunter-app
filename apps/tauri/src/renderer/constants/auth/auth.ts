export type AuthBoardId = 'linkedin' | 'indeed' | 'xing' | 'glassdoor';

export const AUTH_BOARD_IDS = {
  LINKEDIN: 'linkedin' as AuthBoardId,
  INDEED: 'indeed' as AuthBoardId,
  XING: 'xing' as AuthBoardId,
  GLASSDOOR: 'glassdoor' as AuthBoardId,
} as const;

export const AUTH_BOARDS = [
  {
    id: AUTH_BOARD_IDS.LINKEDIN,
    name: 'LinkedIn',
    hint: 'Click Connect to log in manually. The session is stored locally and reused.',
    useSessionAuth: true,
  },
  {
    id: AUTH_BOARD_IDS.INDEED,
    name: 'Indeed',
    hint: 'Click Connect to log in manually (supports email-code 2FA in the popup window).',
    useSessionAuth: true,
  },
  {
    id: AUTH_BOARD_IDS.XING,
    name: 'Xing',
    hint: 'Click Connect to log in manually. Required for the Xing scraper.',
    useSessionAuth: true,
  },
  {
    id: AUTH_BOARD_IDS.GLASSDOOR,
    name: 'Glassdoor',
    hint: 'Click Connect to log in manually. Session is stored for future use.',
    useSessionAuth: true,
  },
] as const;
