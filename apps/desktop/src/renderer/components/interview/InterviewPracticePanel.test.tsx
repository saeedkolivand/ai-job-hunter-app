import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { LikelyQuestion } from '@/lib/generate';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { InterviewPracticePanel, type PracticeFeedbackEntry } from './InterviewPracticePanel';

const questions: LikelyQuestion[] = [
  { id: 'lq-1', question: 'Tell me about a time you led a project.', type: 'behavioral' },
  { id: 'lq-2', question: 'How would you scale our API?', type: 'technical' },
];

const baseProps = {
  questions: [] as LikelyQuestion[],
  generating: false,
  error: null as string | null,
  canGenerate: true,
  canUse: true,
  hasDesc: true,
  onGenerate: vi.fn(),
  feedback: {} as Record<string, PracticeFeedbackEntry>,
  onGetFeedback: vi.fn(),
};

describe('InterviewPracticePanel', () => {
  it('renders the empty state when there are no questions and it is not generating', () => {
    render(<InterviewPracticePanel {...baseProps} />);
    expect(screen.getByText('applications.detail.interview.practice.empty')).toBeInTheDocument();
    expect(screen.getByText('applications.detail.interview.practice.generate')).toBeInTheDocument();
  });

  it('shows a loading skeleton instead of the empty state while generating', () => {
    render(<InterviewPracticePanel {...baseProps} generating />);
    expect(
      screen.queryByText('applications.detail.interview.practice.empty')
    ).not.toBeInTheDocument();
    expect(
      screen.getByText('applications.detail.interview.practice.generating')
    ).toBeInTheDocument();
  });

  it('renders one item per question, each with its type badge and an answer box', () => {
    render(<InterviewPracticePanel {...baseProps} questions={questions} />);
    expect(screen.getByText('Tell me about a time you led a project.')).toBeInTheDocument();
    expect(screen.getByText('How would you scale our API?')).toBeInTheDocument();
    expect(
      screen.getByText('applications.detail.interview.practice.type.behavioral')
    ).toBeInTheDocument();
    expect(
      screen.getByText('applications.detail.interview.practice.type.technical')
    ).toBeInTheDocument();
    expect(screen.getAllByRole('textbox')).toHaveLength(2);
  });

  it('shows the regenerate label once questions exist', () => {
    render(<InterviewPracticePanel {...baseProps} questions={questions} />);
    expect(
      screen.getByText('applications.detail.interview.practice.regenerate')
    ).toBeInTheDocument();
  });

  it('clicking the generate button calls onGenerate', () => {
    const onGenerate = vi.fn();
    render(<InterviewPracticePanel {...baseProps} onGenerate={onGenerate} />);

    fireEvent.click(screen.getByText('applications.detail.interview.practice.generate'));

    expect(onGenerate).toHaveBeenCalledTimes(1);
  });

  it('disables the generate button when canGenerate is false, and surfaces needsModel/needsJob hints', () => {
    render(
      <InterviewPracticePanel {...baseProps} canGenerate={false} canUse={false} hasDesc={false} />
    );
    expect(
      screen.getByText('applications.detail.interview.practice.generate').closest('button')
    ).toBeDisabled();
    expect(screen.getByText('applications.detail.interview.needsModel')).toBeInTheDocument();
  });

  it('surfaces a top-level generation error', () => {
    render(<InterviewPracticePanel {...baseProps} error="boom" />);
    expect(screen.getByText('boom')).toBeInTheDocument();
  });

  it('the get-feedback button stays disabled until the candidate types an answer, then calls onGetFeedback with question + answer', () => {
    const onGetFeedback = vi.fn();
    render(
      <InterviewPracticePanel
        {...baseProps}
        questions={[questions[0] as LikelyQuestion]}
        onGetFeedback={onGetFeedback}
      />
    );

    const getFeedbackButton = screen
      .getByText('applications.detail.interview.practice.getFeedback')
      .closest('button');
    expect(getFeedbackButton).toBeDisabled();

    const textarea = screen.getByRole('textbox');
    fireEvent.change(textarea, { target: { value: 'I led a database migration.' } });

    expect(getFeedbackButton).not.toBeDisabled();
    fireEvent.click(getFeedbackButton as HTMLButtonElement);

    expect(onGetFeedback).toHaveBeenCalledWith(questions[0], 'I led a database migration.');
  });

  it('streams live feedback text while loading, before the rubric parses', () => {
    render(
      <InterviewPracticePanel
        {...baseProps}
        questions={[questions[0] as LikelyQuestion]}
        feedback={{ 'lq-1': { text: 'Strong open', feedback: null, loading: true, error: null } }}
      />
    );
    expect(screen.getByText('Strong open')).toBeInTheDocument();
    expect(
      screen.getByText('applications.detail.interview.practice.gettingFeedback')
    ).toBeInTheDocument();
  });

  it('renders the parsed STAR rubric — strengths, gaps, completeness badges, and the rewrite', () => {
    render(
      <InterviewPracticePanel
        {...baseProps}
        questions={[questions[0] as LikelyQuestion]}
        feedback={{
          'lq-1': {
            text: 'raw',
            loading: false,
            error: null,
            feedback: {
              strengths: ['Clear ownership'],
              gaps: ['No stakeholder mention'],
              star: { situation: true, task: true, action: false, result: true },
              rewrite: 'Tightened rewrite text.',
            },
          },
        }}
      />
    );

    expect(screen.getByText('Clear ownership')).toBeInTheDocument();
    expect(screen.getByText('No stakeholder mention')).toBeInTheDocument();
    expect(screen.getByText('Tightened rewrite text.')).toBeInTheDocument();
    // Present/missing badges render for every STAR field.
    expect(
      screen.getByText(
        /applications\.detail\.interview\.practice\.feedback\.action: applications\.detail\.interview\.practice\.feedback\.missing/
      )
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        /applications\.detail\.interview\.practice\.feedback\.result: applications\.detail\.interview\.practice\.feedback\.present/
      )
    ).toBeInTheDocument();
  });

  it('surfaces a per-question feedback error', () => {
    render(
      <InterviewPracticePanel
        {...baseProps}
        questions={[questions[0] as LikelyQuestion]}
        feedback={{
          'lq-1': { text: '', feedback: null, loading: false, error: 'feedback failed' },
        }}
      />
    );
    expect(screen.getByText('feedback failed')).toBeInTheDocument();
  });

  it('clears a typed answer when the questions array changes (Regenerate) — no stale text carries over', () => {
    const { rerender } = render(
      <InterviewPracticePanel {...baseProps} questions={[questions[0] as LikelyQuestion]} />
    );

    fireEvent.change(screen.getByRole('textbox'), {
      target: { value: 'Stale answer from the previous question set.' },
    });
    expect(screen.getByRole('textbox')).toHaveValue('Stale answer from the previous question set.');

    // Regenerate — a brand-new questions array (new ids, per the id-nonce fix).
    const regenerated: LikelyQuestion[] = [
      { id: '2-lq-1', question: 'Tell me about a time you led a project.', type: 'behavioral' },
    ];
    rerender(<InterviewPracticePanel {...baseProps} questions={regenerated} />);

    expect(screen.getByRole('textbox')).toHaveValue('');
  });
});
