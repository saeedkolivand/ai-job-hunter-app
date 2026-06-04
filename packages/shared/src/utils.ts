/**
 * Format bytes to human-readable string
 */
export function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

/**
 * Format seconds to human-readable time string
 */
export function formatTimeRemaining(seconds: number): string {
  if (seconds < 60) {
    return `${Math.round(seconds)}s`;
  }
  if (seconds < 3600) {
    return `${Math.round(seconds / 60)}m`;
  }
  return `${Math.round(seconds / 3600)}h`;
}

/**
 * Get device tier based on RAM and CPU count
 */
export function getDeviceTier(
  totalRamGb: number,
  cpuCount?: number
): { label: string; color: string } {
  // High-end: 16GB+ RAM OR 8GB+ RAM with 8+ CPU cores
  if (totalRamGb >= 16 || (totalRamGb >= 8 && (cpuCount ?? 0) >= 8)) {
    return { label: 'High-end', color: 'text-emerald-400' };
  }
  // Mid-range: 8GB+ RAM OR 4GB+ RAM with 4+ CPU cores
  if (totalRamGb >= 8 || (totalRamGb >= 4 && (cpuCount ?? 0) >= 4)) {
    return { label: 'Mid-range', color: 'text-blue-400' };
  }
  // Low-end: Everything else
  return { label: 'Low-end', color: 'text-amber-400' };
}

/**
 * Calculate download speed from byte delta over time
 * Returns speed in bytes per second, or 0 if calculation is not possible
 */
export function calculateDownloadSpeed(
  currentBytes: number,
  previousBytes: number,
  currentTimeMs: number,
  previousTimeMs: number
): number {
  const bytesDiff = currentBytes - previousBytes;
  const timeDiff = (currentTimeMs - previousTimeMs) / 1000; // Convert to seconds

  // Only calculate if we have meaningful data
  if (timeDiff < 0.05 || bytesDiff <= 0) {
    return 0;
  }

  return bytesDiff / timeDiff;
}

/**
 * Format download speed to human-readable string
 */
export function formatDownloadSpeed(bytesPerSecond: number): string {
  if (!isFinite(bytesPerSecond) || bytesPerSecond <= 0) {
    return '';
  }
  if (bytesPerSecond >= 1024 * 1024) {
    return `${(bytesPerSecond / (1024 * 1024)).toFixed(1)} MB/s`;
  }
  return `${(bytesPerSecond / 1024).toFixed(1)} KB/s`;
}

/**
 * Calculate estimated time remaining for download
 */
export function calculateTimeRemaining(
  totalBytes: number,
  downloadedBytes: number,
  bytesPerSecond: number
): number {
  if (!isFinite(bytesPerSecond) || bytesPerSecond <= 0 || totalBytes <= 0) {
    return 0;
  }
  const remainingBytes = totalBytes - downloadedBytes;
  if (remainingBytes <= 0) {
    return 0;
  }
  return remainingBytes / bytesPerSecond;
}

/**
 * Compare two semantic version strings (`MAJOR.MINOR.PATCH`).
 *
 * Tolerant of a leading `v` and of a `-prerelease`/`+build` suffix (ignored for
 * ordering — release lines only ever surface stable tags). Missing or malformed
 * numeric segments are treated as `0`. Returns a negative number when `a < b`,
 * `0` when equal, and a positive number when `a > b`, so it slots straight into
 * `Array.prototype.sort`.
 */
export function compareSemver(a: string, b: string): number {
  const parse = (v: string): [number, number, number] => {
    const core = v.trim().replace(/^v/i, '').split(/[-+]/, 1)[0] ?? '';
    const [major = 0, minor = 0, patch = 0] = core
      .split('.')
      .map((n) => Number.parseInt(n, 10) || 0);
    return [major, minor, patch];
  };
  const [aMajor, aMinor, aPatch] = parse(a);
  const [bMajor, bMinor, bPatch] = parse(b);
  return aMajor - bMajor || aMinor - bMinor || aPatch - bPatch;
}
