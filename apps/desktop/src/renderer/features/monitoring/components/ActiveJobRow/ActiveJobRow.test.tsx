import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { JobRecord } from '@/features/monitoring/types';

import { ActiveJobRow } from './index';

vi.mock('@ajh/ui', () => ({
  cn: (...a: unknown[]) => a.filter(Boolean).join(' '),
  transition: {},
}));
vi.mock('motion/react', () => ({
  motion: { div: ({ children }: { children: React.ReactNode }) => <div>{children}</div> },
}));

const t = (k: string) => `T:${k}`;
const job = (status: JobRecord['status']) =>
  ({ id: '1', kind: 'ai.generate', status, progress: 0 }) as unknown as JobRecord;

describe('ActiveJobRow — status is translated, never the raw enum', () => {
  it.each([
    ['queued', 'monitoring.activity.queued'],
    ['running', 'monitoring.metrics.running'],
    ['streaming', 'monitoring.timeLabels.streaming'],
  ] as const)('%s -> %s', (status, key) => {
    render(<ActiveJobRow job={job(status)} kindLabel={{}} t={t} />);
    expect(screen.getByText(`T:${key}`)).toBeInTheDocument();
    expect(screen.queryByText(status)).not.toBeInTheDocument();
  });
});
