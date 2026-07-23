// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/react';

import { CookieGag } from './CookieGag';

afterEach(() => {
  cleanup();
});

describe('CookieGag', () => {
  it('renders the cookie notice with its dismiss button', () => {
    const { container } = render(<CookieGag />);
    expect(container.querySelector('#cookie')).not.toBeNull();
    expect(container.querySelector('#cookie button')?.textContent).toBe('ok');
  });

  it('removes #cookie from the DOM when the dismiss button is clicked', () => {
    const { unmount } = render(<CookieGag />);
    expect(document.getElementById('cookie')).not.toBeNull();

    fireEvent.click(document.querySelector('#cookie button') as HTMLButtonElement);

    expect(document.getElementById('cookie')).toBeNull();
    try {
      // CookieGag's root IS the #cookie node the click just detached natively,
      // so React's own unmount (run by afterEach's cleanup()) can't remove it
      // a second time — swallow that expected NotFoundError here instead of
      // letting it surface (misattributed) against the next test.
      unmount();
    } catch {
      // expected — see comment above
    }
  });
});
