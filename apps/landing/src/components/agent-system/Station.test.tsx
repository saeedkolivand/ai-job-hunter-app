// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import type { Station } from '@/data/agent-fleet';
import { STATION_COUNT } from '@/lib/agent-system/belt';

import { StationView } from './Station';

afterEach(cleanup);

const WITH_TAG: Station = {
  title: 'author implements',
  access: 'write access',
  desc: 'the domain author makes the smallest diff that fits.',
  agentTag: 'rust-backend-author',
  machine: 'pen',
  stamp: '✎',
};

const WITHOUT_TAG: Station = {
  title: 'intake & triage',
  access: 'main session',
  desc: 'paths matched to review-routes.json; first glob wins.',
  agentTag: '',
  machine: 'router',
  stamp: '·',
};

describe('StationView — horizontal layout', () => {
  it('renders the station-index label, title, desc, and lit only on index 0', () => {
    const { container } = render(<StationView station={WITH_TAG} index={1} layout="horizontal" />);
    const el = container.querySelector('.station');
    if (!el) throw new Error('expected .station');

    expect(el.getAttribute('data-i')).toBe('1');
    expect(el.classList.contains('lit')).toBe(false);
    expect(el.querySelector('.sn')?.textContent).toBe(`2 / ${STATION_COUNT}`);
    expect(el.querySelector('.st')?.textContent).toBe(WITH_TAG.title);
    expect(el.querySelector('.sd')?.textContent).toBe(WITH_TAG.desc);
    expect(el.querySelector('.machine')).toBeTruthy();
    expect(el.querySelector('.post')).toBeTruthy();
  });

  it('is lit only when index === 0', () => {
    const { container } = render(<StationView station={WITH_TAG} index={0} layout="horizontal" />);
    expect(container.querySelector('.station')?.classList.contains('lit')).toBe(true);
  });

  it.each([
    ['renders an .agent-tag when agentTag is set', WITH_TAG, true],
    ['renders no .agent-tag when agentTag is empty', WITHOUT_TAG, false],
  ] as const)('%s', (_desc, station, expectTag) => {
    const { container } = render(<StationView station={station} index={0} layout="horizontal" />);
    const tag = container.querySelector('.agent-tag');
    expect(Boolean(tag)).toBe(expectTag);
    if (expectTag) expect(tag?.textContent).toBe(station.agentTag);
  });
});

describe('StationView — vertical layout', () => {
  it('renders as an <li> with the vertical n/title/desc structure the CSS/scripts rely on', () => {
    const { container } = render(
      <ol>
        <StationView station={WITH_TAG} index={1} layout="vertical" />
      </ol>
    );
    const li = container.querySelector('li');
    if (!li) throw new Error('expected an <li>');

    expect(li.querySelector('.vmachine')).toBeTruthy();
    expect(li.querySelector('.vbody')).toBeTruthy();
    expect(li.querySelector('.vn')?.textContent).toBe(
      `station 2 / ${STATION_COUNT} · ${WITH_TAG.access}`
    );
    expect(li.querySelector('.vt')?.textContent).toBe(WITH_TAG.title);
    expect(li.querySelector('.vd')?.textContent).toBe(WITH_TAG.desc);
    // The vertical layout never carries the horizontal-only "lit"/data-i wiring.
    expect(container.querySelector('.station')).toBeNull();
  });

  it.each([
    ['renders an .agent-tag when agentTag is set', WITH_TAG, true],
    ['renders no .agent-tag when agentTag is empty', WITHOUT_TAG, false],
  ] as const)('%s', (_desc, station, expectTag) => {
    const { container } = render(
      <ol>
        <StationView station={station} index={0} layout="vertical" />
      </ol>
    );
    const tag = container.querySelector('.agent-tag');
    expect(Boolean(tag)).toBe(expectTag);
    if (expectTag) expect(tag?.textContent).toBe(station.agentTag);
  });
});
