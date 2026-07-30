// First-load placeholders so the page never shows a big blank gap while the
// GitHub API resolves. Decorative (aria-hidden); the busy state is announced once.
function SkeletonGrid({ n = 4 }: { n?: number }) {
  return (
    <div className="mc-grid" aria-hidden="true">
      {Array.from({ length: n }, (_, i) => (
        <div className="mc-card mc-skeleton" key={i}>
          <div className="mc-skel" style={{ height: '34px', width: '58%', marginBottom: '10px' }} />
          <div className="mc-skel" style={{ height: '10px', width: '80%' }} />
        </div>
      ))}
    </div>
  );
}

export function MissionControlSkeleton() {
  return (
    <div aria-busy="true" aria-label="Loading whole-repo state">
      <div className="mc-verdict mc-skeleton" aria-hidden="true">
        <div className="mc-skel" style={{ height: '16px', width: '120px', marginBottom: '10px' }} />
        <div className="mc-skel" style={{ height: '30px', width: '70%' }} />
      </div>
      <div className="mc-section" aria-hidden="true">
        <div className="mc-skel" style={{ height: '26px', width: '180px', marginBottom: '16px' }} />
        <SkeletonGrid />
      </div>
      <div className="mc-section" aria-hidden="true">
        <div className="mc-skel" style={{ height: '26px', width: '150px', marginBottom: '16px' }} />
        <SkeletonGrid n={3} />
      </div>
    </div>
  );
}
