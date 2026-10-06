import { z } from 'zod';

import { AUTH_CAPABLE_BOARDS } from '../types/index.js';

export const JobIdSchema = z.object({ jobId: z.string().min(1) });

export const CredentialSetSchema = z.object({
  boardId: z.enum(AUTH_CAPABLE_BOARDS),
  username: z.string().min(1).max(254),
  password: z.string().min(1).max(512),
});

export const CredentialBoardSchema = z.object({
  boardId: z.enum(AUTH_CAPABLE_BOARDS),
});

export type CredentialSetRequest = z.infer<typeof CredentialSetSchema>;
export type CredentialBoardRequest = z.infer<typeof CredentialBoardSchema>;
