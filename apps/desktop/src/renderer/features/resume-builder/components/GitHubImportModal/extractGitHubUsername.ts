/** Extract a bare GitHub username from a full profile URL or return the input as-is. */
export function extractGitHubUsername(value: string): string {
  try {
    const url = new URL(value);
    if (url.hostname === 'github.com' || url.hostname === 'www.github.com') {
      const seg = url.pathname.replace(/^\//, '').split('/')[0];
      return seg ?? value;
    }
  } catch {
    // not a URL — treat as bare username
  }
  return value;
}
