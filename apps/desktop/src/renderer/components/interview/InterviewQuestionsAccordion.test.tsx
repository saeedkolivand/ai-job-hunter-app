import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { InterviewQuestion } from '@ajh/shared';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/lib/generate', () => ({
  INTERVIEW_AUDIENCES: ['recruiter', 'hiringManager', 'team', 'leadership', 'general'],
}));

// Collapse motion so the open panel's content renders synchronously.
vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

import { InterviewQuestionsAccordion } from './InterviewQuestionsAccordion';

const q = (id: string, audience: string, question: string): InterviewQuestion => ({
  id,
  audience,
  question,
  why: '',
});

describe('InterviewQuestionsAccordion', () => {
  it('renders one section per audience present, with label · count titles, in canonical order', () => {
    render(
      <InterviewQuestionsAccordion
        questions={[
          q('1', 'recruiter', 'How is the team structured?'),
          q('2', 'recruiter', 'What does onboarding look like for this role?'),
          q('3', 'team', 'How do you handle code review here?'),
        ]}
      />
    );

    // Section headers carry the audience label and its question count.
    expect(
      screen.getByText('applications.detail.interview.audience.recruiter · 2')
    ).toBeInTheDocument();
    expect(screen.getByText('applications.detail.interview.audience.team · 1')).toBeInTheDocument();
    // Audiences with no questions are not rendered.
    expect(screen.queryByText(/audience\.leadership/)).not.toBeInTheDocument();

    // The first section is open by default, so its questions are visible.
    expect(screen.getByText('How is the team structured?')).toBeInTheDocument();
  });

  it('renders nothing when there are no questions', () => {
    const { container } = render(<InterviewQuestionsAccordion questions={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});
