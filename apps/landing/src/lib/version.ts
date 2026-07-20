// Installer-URL shape + semver comparison for the /download version seam.
//
// KEEP IN SYNC with scripts/sync-download-page.cjs (`buildInstallers`): the
// release pipeline computes the same URLs to write src/data/version.json, and
// the client freshness check recomputes them here for a newer GitHub release.

export interface Installers {
  macArm: string;
  macIntel: string;
  winExe: string;
  winMsi: string;
  linuxAppImage: string;
  linuxDeb: string;
  linuxRpm: string;
}

export interface VersionData {
  version: string;
  /** ISO-8601 release timestamp, or null when unknown (seed / pre-release). */
  releasedAt: string | null;
  installers: Installers;
}

const REPO = 'https://github.com/saeedkolivand/ai-job-hunter-app';

/** Per-OS GitHub Release asset URLs, pinned to `version` (no leading `v`). */
export function buildInstallers(version: string): Installers {
  const base = `${REPO}/releases/download/v${version}`;
  return {
    macArm: `${base}/macos-AI-Job-Hunter_${version}_aarch64-apple-silicon.dmg`,
    macIntel: `${base}/macos-AI-Job-Hunter_${version}_x64-intel.dmg`,
    winExe: `${base}/windows-AI-Job-Hunter_${version}_x64-setup.exe`,
    winMsi: `${base}/windows-AI-Job-Hunter_${version}_x64_en-US.msi`,
    linuxAppImage: `${base}/linux-AI-Job-Hunter_${version}_amd64.AppImage`,
    linuxDeb: `${base}/linux-AI-Job-Hunter_${version}_amd64.deb`,
    linuxRpm: `${base}/linux-AI-Job-Hunter-${version}-1.x86_64.rpm`,
  };
}

// [major, minor, patch] — a leading `v` and any `-`/`.` pre-release suffix are
// dropped; non-numeric or missing segments clamp to 0.
function triple(v: string): [number, number, number] {
  const core = v.replace(/^v/, '').split(/[.-]/);
  const seg = (i: number): number => {
    const n = Number.parseInt(core[i] ?? '0', 10);
    return Number.isFinite(n) ? n : 0;
  };
  return [seg(0), seg(1), seg(2)];
}

/** True when `candidate` is a strictly higher release than `baseline`. */
export function isNewer(candidate: string, baseline: string): boolean {
  const a = triple(candidate);
  const b = triple(baseline);
  for (let i = 0; i < 3; i++) {
    const ai = a[i] ?? 0;
    const bi = b[i] ?? 0;
    if (ai !== bi) return ai > bi;
  }
  return false;
}
