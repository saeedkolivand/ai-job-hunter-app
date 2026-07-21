// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { clusters, edges, nodes } from '@/data/architecture-map';

import { ArchitectureMap } from './ArchitectureMap';

afterEach(() => {
  cleanup();
});

// Render-smoke only — the pan/zoom/drag interaction engine reads
// getBoundingClientRect (jsdom zeroes it), so that's covered by live QA, not
// here. This just proves the static SVG structure mounts 1:1 with the data.
describe('ArchitectureMap', () => {
  it('mounts without throwing and renders one element per node/cluster/edge', () => {
    const { container } = render(<ArchitectureMap />);
    expect(container.querySelectorAll('.node')).toHaveLength(nodes.length);
    expect(container.querySelectorAll('.clusterBox')).toHaveLength(clusters.length);
    expect(container.querySelectorAll('path.edge')).toHaveLength(edges.length);
  });

  it('renders the default sidebar overview on mount', () => {
    render(<ArchitectureMap />);
    expect(screen.getByRole('heading', { name: 'AI Job Hunter — architecture' })).toBeTruthy();
  });
});
