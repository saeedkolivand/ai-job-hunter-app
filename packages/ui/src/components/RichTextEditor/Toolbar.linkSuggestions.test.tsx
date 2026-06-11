import { describe, expect, it } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { RichTextEditor } from './RichTextEditor';
import type { LinkSuggestion } from './Toolbar';

const LINKEDIN: LinkSuggestion = { label: 'LinkedIn', url: 'https://www.linkedin.com/in/jane-doe' };
const GITHUB: LinkSuggestion = { label: 'GitHub', url: 'https://github.com/janedoe' };
const EMAIL: LinkSuggestion = { label: 'Email', url: 'mailto:jane@example.com' };
const SUGGESTIONS: LinkSuggestion[] = [LINKEDIN, GITHUB, EMAIL];

/** Open the link dialog by clicking the Link toolbar button. */
async function openLinkDialog(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Link' }));
  return screen.getByRole('dialog');
}

/** The URL <input type="url"> inside the dialog. */
function urlField(): HTMLInputElement {
  // Two inputs in the dialog (Text, URL); the URL one is type="url".
  const inputs = screen.getAllByRole('textbox') as HTMLInputElement[];
  const url = inputs.find((el) => el.getAttribute('type') === 'url');
  if (!url) throw new Error('URL field not found');
  return url;
}

describe('RichTextEditor link dialog — suggestions', () => {
  it('renders no suggestions section when linkSuggestions is empty', async () => {
    const user = userEvent.setup();
    render(<RichTextEditor value="hello world" onChange={() => {}} />);
    await openLinkDialog(user);
    expect(screen.queryByText('Your links')).not.toBeInTheDocument();
  });

  it('renders a row per suggestion with an accessible name of "label — url"', async () => {
    const user = userEvent.setup();
    render(
      <RichTextEditor value="hello world" onChange={() => {}} linkSuggestions={SUGGESTIONS} />
    );
    await openLinkDialog(user);
    expect(screen.getByText('Your links')).toBeInTheDocument();
    for (const s of SUGGESTIONS) {
      expect(screen.getByRole('button', { name: `${s.label} — ${s.url}` })).toBeInTheDocument();
    }
  });

  it('clicking a row fills the URL field and the (empty) label field', async () => {
    const user = userEvent.setup();
    render(
      <RichTextEditor value="hello world" onChange={() => {}} linkSuggestions={SUGGESTIONS} />
    );
    await openLinkDialog(user);

    const labelField = screen.getByRole('textbox', { name: 'Text' }) as HTMLInputElement;
    expect(labelField.value).toBe('');

    await user.click(screen.getByRole('button', { name: `${GITHUB.label} — ${GITHUB.url}` }));

    expect(urlField().value).toBe(GITHUB.url);
    // Label was empty → it gets filled from the picked suggestion.
    expect(labelField.value).toBe('GitHub');
  });

  it('typing in the URL field narrows the list (case-insensitive substring)', async () => {
    const user = userEvent.setup();
    render(
      <RichTextEditor value="hello world" onChange={() => {}} linkSuggestions={SUGGESTIONS} />
    );
    await openLinkDialog(user);

    await user.type(urlField(), 'GITHUB');

    const list = screen.getByRole('list');
    expect(within(list).getByText('GitHub')).toBeInTheDocument();
    expect(within(list).queryByText('LinkedIn')).not.toBeInTheDocument();
    expect(within(list).queryByText('Email')).not.toBeInTheDocument();
  });

  it('hides the suggestions section when the filter matches nothing', async () => {
    const user = userEvent.setup();
    render(
      <RichTextEditor value="hello world" onChange={() => {}} linkSuggestions={SUGGESTIONS} />
    );
    await openLinkDialog(user);

    await user.type(urlField(), 'zzz-no-match');

    expect(screen.queryByText('Your links')).not.toBeInTheDocument();
  });
});
