export type AuthBoardId = 'linkedin';

export const AUTH_BOARD_IDS = {
  LINKEDIN: 'linkedin' as AuthBoardId,
} as const;

export const AUTH_BOARDS = [
  {
    id: AUTH_BOARD_IDS.LINKEDIN,
    name: 'LinkedIn',
    hint: 'Click Connect to log in manually. The session is stored locally and reused.',
    useSessionAuth: true,
  },
] as const;
