import { describe, expect, it } from 'vitest';

import { extractPlainText } from '../index';

describe('extractPlainText', () => {
  it('strips think blocks, markdown headers and inline code', () => {
    const raw = '<think>internal reasoning</think>\n# Heading\nSome text with `code` here.';
    const out = extractPlainText(raw);
    expect(out).not.toContain('<think>');
    expect(out).not.toContain('internal reasoning');
    expect(out).not.toContain('# Heading');
    expect(out).toContain('Heading');
    expect(out).not.toContain('`code`');
    expect(out).toContain('code');
  });

  it('strips XML wrapper tags echoed from the prompt', () => {
    const out = extractPlainText('<candidate_resume>body</candidate_resume>');
    expect(out).not.toContain('<candidate_resume>');
    expect(out).toContain('body');
  });

  it('reduces emphasis markers (triple collapses to bold, single italic is stripped)', () => {
    expect(extractPlainText('***strong***')).toBe('**strong**');
    expect(extractPlainText('an *italic* word')).toBe('an italic word');
  });

  it('leaves **bold** keyword markup intact', () => {
    // The italic pass used to match the INNER pair of a bold run, downgrading
    // `**bold**` to `*bold*`; and since `[^*]+` matches spaces and commas, two
    // adjacent bold spans paired up across each other and swallowed the text
    // between them. The prompts ask for 2-3 `**keyword**` bolds per bullet, so
    // this corrupted essentially every generated document.
    expect(extractPlainText('**bold**')).toBe('**bold**');
    expect(extractPlainText('Skills: **Python**, **Go**, **Kubernetes**')).toBe(
      'Skills: **Python**, **Go**, **Kubernetes**'
    );
    // Bold and italic in the same line: only the italic is stripped.
    expect(extractPlainText('**a** and *b*')).toBe('**a** and b');
  });

  it('preserves a markdown `*`-bullet list across lines', () => {
    // Regression: `[^*]+` in the italic-strip pass spanned newlines, so the
    // leading `*` of one bullet paired with the leading `*` of the NEXT
    // bullet (matching across the `\n`) and both list markers were eaten,
    // leaving ` apple\n banana`.
    const out = extractPlainText('* apple\n* banana');
    expect(out).toBe('* apple\n* banana');
  });

  it('still strips a single-line italic span', () => {
    expect(extractPlainText('This is *italic* text.')).toBe('This is italic text.');
  });

  it('removes a fenced code block entirely (no orphaned backticks or code leak)', () => {
    // Regression: the inline-backtick pass used to consume the ``` fence markers
    // first, so the fenced regex could not match and the code body leaked.
    const out = extractPlainText('Intro.\n```\nconst x = 1;\n```\nOutro.');
    // The fence (and only the fence) is gone — surrounding prose is preserved.
    // The minimal reorder fix leaves the blank line where the fence stood; what
    // matters is no backticks survive and the code body does not leak.
    expect(out).not.toContain('```');
    expect(out).not.toContain('const x = 1;');
    expect(out).toContain('Intro.');
    expect(out).toContain('Outro.');
    expect(out.replace(/\n+/g, '\n')).toBe('Intro.\nOutro.');
  });

  it('strips a language-tagged fenced block too', () => {
    const out = extractPlainText('Before\n```ts\nlet y = 2;\n```\nAfter');
    expect(out).not.toContain('let y = 2;');
    expect(out).not.toContain('```');
    expect(out).toContain('Before');
    expect(out).toContain('After');
  });

  it('still strips inline single-backtick code spans', () => {
    const out = extractPlainText('Use the `npm install` command.');
    expect(out).toBe('Use the npm install command.');
    expect(out).not.toContain('`');
  });

  describe('whole-response code fence (HIGH — a local model that wraps its ENTIRE answer in one fence must not have the whole document deleted, the same Ollama tell already fixed in parseGitHubProjects)', () => {
    it('unwraps a bare-fenced whole résumé instead of deleting it', () => {
      const raw =
        '```\nJohn Doe\nSenior Engineer\n\nPROFESSIONAL SUMMARY\nBuilt lots of things.\n```';
      const out = extractPlainText(raw);
      expect(out).not.toBe('');
      expect(out).not.toContain('```');
      expect(out).toContain('John Doe');
      expect(out).toContain('PROFESSIONAL SUMMARY');
    });

    it('unwraps a language-tagged (```markdown) whole résumé instead of deleting it', () => {
      const raw = '```markdown\nJohn Doe\nSenior Engineer\n\nBuilt lots of things.\n```';
      const out = extractPlainText(raw);
      expect(out).not.toBe('');
      expect(out).not.toContain('```');
      expect(out).toContain('John Doe');
    });

    it('unwraps a whole-fenced cover letter with a trailing newline instead of deleting it', () => {
      const raw = '```\nDear Hiring Manager,\n\nI am writing to apply.\n\nSincerely,\nJane\n```\n';
      const out = extractPlainText(raw);
      expect(out).not.toBe('');
      expect(out).not.toContain('```');
      expect(out).toContain('Dear Hiring Manager,');
      expect(out).toContain('Sincerely,');
    });

    it('unwraps a ONE-LINE whole-response fence (no interior newline) instead of emptying it', () => {
      // The original fix only matched a fence whose opening marker was
      // followed by a newline, so a short answer the model wrapped on a
      // single line fell straight through to the delete pass and came back
      // as ''. A one-word application answer is exactly that shape.
      expect(extractPlainText('```Yes.```')).toBe('Yes.');
      expect(extractPlainText('```I have 5 years of TypeScript experience.```')).toBe(
        'I have 5 years of TypeScript experience.'
      );
    });

    it('leaves two back-to-back one-line fences to the delete pass', () => {
      // Not a single whole-answer wrap — the interior-fence guard must still
      // refuse to guess which of the two blocks is "the" answer.
      const out = extractPlainText('```a``` and ```b```');
      // Assert the exact result, not just the absence of backticks: stripping
      // only the delimiters and leaving `a`/`b` behind would also satisfy a
      // `not.toContain('```')` check.
      expect(out).toBe('and');
    });

    it('still deletes a genuine fenced code block embedded mid-answer (not the whole response)', () => {
      // This is the differential the fix must preserve: only a fence spanning
      // the ENTIRE trimmed response is unwrapped. A fence that is part of a
      // larger answer is still noise to strip, per the existing behaviour
      // pinned by the two tests above this describe block.
      const raw = 'Here is an example:\n\n```js\nconst x = 1;\n```\n\nThat is how you would do it.';
      const out = extractPlainText(raw);
      expect(out).not.toContain('```');
      expect(out).not.toContain('const x = 1;');
      expect(out).toContain('Here is an example:');
      expect(out).toContain('That is how you would do it.');
    });
  });
});
