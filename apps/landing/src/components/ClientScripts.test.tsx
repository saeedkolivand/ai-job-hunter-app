// @vitest-environment jsdom
import { StrictMode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { ClientScripts } from './ClientScripts';

const SRCS = ['/scripts/a.js', '/scripts/b.js'];

function gagScripts(): HTMLScriptElement[] {
  return Array.from(document.querySelectorAll<HTMLScriptElement>('script[data-gag]'));
}

afterEach(() => {
  cleanup();
  document.body.innerHTML = '';
});

describe('ClientScripts', () => {
  it('leaves exactly one script[data-gag] node per src under StrictMode double-invoke', () => {
    render(
      <StrictMode>
        <ClientScripts srcs={SRCS} />
      </StrictMode>
    );

    expect(gagScripts().map((el) => el.dataset.gag)).toEqual(SRCS);
  });

  it('removes every appended node on unmount', () => {
    const { unmount } = render(<ClientScripts srcs={SRCS} />);
    expect(gagScripts()).toHaveLength(SRCS.length);

    unmount();

    expect(gagScripts()).toHaveLength(0);
  });
});
