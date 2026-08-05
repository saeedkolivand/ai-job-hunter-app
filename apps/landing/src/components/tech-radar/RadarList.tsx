import {
  QUADRANTS,
  RADAR,
  type RadarSubjectKind,
  RINGS,
  type TechRadarEntry,
} from '@/data/tech-radar';
import { GITHUB_REPO } from '@/lib/site-links';

// The authoritative, always-rendered accessible content: every entry as real
// text, grouped by quadrant then ring. This is what a screen reader, a
// non-JS user, or a narrow phone viewport actually gets — the SVG in
// <RadarSvg> is a visual index on top of this, never a replacement for it.
// Anchor ids (`tr-entry-<id>`) are what the SVG blips link to.

const SUBJECT_NOTE: Record<RadarSubjectKind, (entry: TechRadarEntry) => string> = {
  dependency: (e) =>
    `Tracked as a dependency named "${e.dependencyName ?? e.name}" — a CI check fails the build if that name ever disappears from every package.json / Cargo.toml in the repo.`,
  technique: () =>
    'An in-house pattern or practice, not a package — not tracked by the dependency check.',
  service: () => 'A hosted or locally-run service, not installed via a package manager.',
  'not-adopted': () =>
    'Names a real package or product that was deliberately never added (or removed on purpose).',
};

function RadarEntryItem({ entry, index }: { entry: TechRadarEntry; index: number }) {
  const ring = RINGS.find((r) => r.id === entry.ring);
  return (
    <li id={`tr-entry-${entry.id}`} className="tr-entry" tabIndex={-1}>
      <div className="tr-entry__head">
        <span className="tr-entry__num" aria-hidden="true">
          {index + 1}
        </span>
        <h4 className="tr-entry__name">{entry.name}</h4>
        <span className={`tr-badge tr-badge--${entry.ring}`}>{ring?.label ?? entry.ring}</span>
      </div>
      <p className="tr-entry__summary">{entry.summary}</p>
      <p className="tr-entry__rationale">{entry.rationale}</p>
      <p className="tr-entry__meta">
        {SUBJECT_NOTE[entry.subjectKind](entry)}
        {entry.adrSlug ? (
          <>
            {' '}
            See{' '}
            <a href={`${GITHUB_REPO}/blob/main/docs/adr/${entry.adrSlug}.md`}>
              {entry.adrSlug.replace(/^0*(\d+)-/, 'ADR-$1: ').replace(/-/g, ' ')}
            </a>
            .
          </>
        ) : null}{' '}
        Reviewed {entry.lastReviewed}.
      </p>
    </li>
  );
}

export function RadarList() {
  const indexById = new Map(RADAR.map((e, i) => [e.id, i]));

  return (
    <div className="tr-list" aria-label="Full technology radar, listed as text">
      {QUADRANTS.map((quadrant) => {
        const entries = RADAR.filter((e) => e.quadrant === quadrant.id);
        return (
          <section key={quadrant.id} className="tr-quadrant-group">
            <h2 className="tr-quadrant-group__title">{quadrant.label}</h2>
            {RINGS.map((ring) => {
              const ringEntries = entries.filter((e) => e.ring === ring.id);
              if (ringEntries.length === 0) return null;
              return (
                <div key={ring.id} className="tr-ring-group">
                  <h3 className="tr-ring-group__title">
                    <span className={`tr-badge tr-badge--${ring.id}`}>{ring.label}</span>
                    <span className="tr-ring-group__blurb">{ring.blurb}</span>
                  </h3>
                  <ul className="tr-entry-list">
                    {ringEntries.map((entry) => (
                      <RadarEntryItem
                        key={entry.id}
                        entry={entry}
                        index={indexById.get(entry.id) ?? 0}
                      />
                    ))}
                  </ul>
                </div>
              );
            })}
          </section>
        );
      })}
    </div>
  );
}
