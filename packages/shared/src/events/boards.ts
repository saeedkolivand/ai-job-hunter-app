export const BOARDS_EVENTS = {
  loginStatus: 'boards:login-status',
} as const;

export interface BoardsLoginStatusEvent {
  boardId: string;
  status: string;
}
