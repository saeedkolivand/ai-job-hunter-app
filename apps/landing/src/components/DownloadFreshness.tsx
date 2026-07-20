'use client';

import { useEffect } from 'react';

import { buildInstallers, isNewer } from '@/lib/version';

const RELEASES_API = 'https://api.github.com/repos/saeedkolivand/ai-job-hunter-app/releases/latest';

// Best-effort freshness: the /download page bakes the version from
// src/data/version.json at build. On mount we ask the GitHub Releases API for
// the latest release; if it is newer than the baked one, we swap the displayed
// version label + the installer hrefs in place (the copy-cmd chips are version-
// independent, so their listeners survive). Any failure is a silent no-op — the
// baked version stays.
export function DownloadFreshness({ baked }: { baked: string }) {
  useEffect(() => {
    let cancelled = false;

    void (async () => {
      try {
        const res = await fetch(RELEASES_API, {
          headers: { Accept: 'application/vnd.github+json' },
        });
        if (!res.ok) return;
        const json: unknown = await res.json();
        const tag =
          typeof json === 'object' && json !== null && 'tag_name' in json
            ? String((json as { tag_name: unknown }).tag_name)
            : '';
        const remote = tag.replace(/^v/, '');
        if (!remote || cancelled || !isNewer(remote, baked)) return;

        const block = document.getElementById('downloads-block');
        if (!block) return;

        const label = block.querySelector('.dl-version b');
        if (label) label.textContent = `v${remote}`;

        const i = buildInstallers(remote);
        const urls = [
          i.macArm,
          i.macIntel,
          i.winExe,
          i.winMsi,
          i.linuxAppImage,
          i.linuxDeb,
          i.linuxRpm,
        ];
        block.querySelectorAll<HTMLAnchorElement>('.dl-btn').forEach((a, idx) => {
          const url = urls[idx];
          if (url) a.href = url;
        });
      } catch {
        // silent — freshness is best-effort; the baked version remains.
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [baked]);

  return null;
}
