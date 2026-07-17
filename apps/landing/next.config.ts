import path from "node:path";
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Static export for GitHub Pages: `next build` emits ./out (HTML/CSS/JS).
  output: "export",
  // Companion to `output: export` -- the default image optimizer needs a server.
  images: { unoptimized: true },
  // This app lives in a pnpm workspace; point file tracing at the repo root so
  // Next resolves hoisted deps instead of walking out of the monorepo.
  outputFileTracingRoot: path.join(__dirname, "../../"),
};

export default nextConfig;
