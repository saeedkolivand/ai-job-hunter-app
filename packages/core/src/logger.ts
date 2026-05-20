import pino, { type Logger as PinoLogger } from 'pino';

export type Logger = PinoLogger;

export function createLogger(name: string, level: pino.Level = 'info'): Logger {
  const isDev = process.env.NODE_ENV !== 'production';
  return pino({
    name,
    level,
    base: { pid: process.pid },
    timestamp: pino.stdTimeFunctions.isoTime,
    ...(isDev
      ? {
          transport: {
            target: 'pino-pretty',
            options: { colorize: true, translateTime: 'SYS:HH:MM:ss.l' },
          },
        }
      : {}),
  });
}
