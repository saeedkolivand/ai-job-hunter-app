'use client';

import { useState } from 'react';

import { PAT_SCOPE_GUIDANCE } from '@/lib/mission-control/pat';

// PAT sign-in / sign-out. The token input is type=password (never rendered as
// text), autoComplete off. The banner states the storage + transport contract
// explicitly; sign-out wipes. All persistence is delegated to pat.ts.
export function SignInPanel({
  signedIn,
  onSignIn,
  onSignOut,
}: {
  signedIn: boolean;
  onSignIn: (token: string) => void;
  onSignOut: () => void;
}) {
  const [value, setValue] = useState('');

  if (signedIn) {
    return (
      <section className="mc-signin" aria-label="GitHub sign-in">
        <p className="mc-signin__title">Signed in</p>
        <p className="mc-banner" role="note">
          Your token lives ONLY in this browser&rsquo;s localStorage and is sent ONLY as an
          Authorization header to api.github.com — never to this site (there is no server). Sign out
          to wipe it.
        </p>
        <button type="button" className="mc-btn" onClick={onSignOut}>
          Sign out &amp; wipe token
        </button>
      </section>
    );
  }

  return (
    <section className="mc-signin" aria-label="GitHub sign-in">
      <p className="mc-signin__title">Sign in for the safe tier (optional)</p>
      <p className="mc-banner" role="note">
        Paste a <b>fine-grained PAT</b> to raise the rate limit to 5,000/h and unlock safe write
        actions. It is stored ONLY in this browser (localStorage) and sent ONLY to api.github.com as
        a Bearer header — never to this site. Sign out wipes it.
      </p>
      <ul className="mc-scopes">
        {PAT_SCOPE_GUIDANCE.map((scope) => (
          <li key={scope.label}>
            <b>{scope.label}:</b> {scope.value}
          </li>
        ))}
      </ul>
      <p className="mc-scope-note">
        This scope list is <b>advisory</b> — the dashboard cannot read, verify, or limit the actual
        permissions of the token you paste. Mint it with least privilege on GitHub.
      </p>
      <form
        className="mc-field"
        onSubmit={(event) => {
          event.preventDefault();
          onSignIn(value);
          setValue('');
        }}
      >
        <label htmlFor="mc-token">Fine-grained personal access token</label>
        <input
          id="mc-token"
          className="mc-input"
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={value}
          onChange={(event) => setValue(event.target.value)}
          placeholder="github_pat_…"
        />
        <button type="submit" className="mc-btn is-primary" disabled={value.trim().length === 0}>
          Save token in this browser
        </button>
      </form>
    </section>
  );
}
