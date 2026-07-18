import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

// check:parity is a standalone node script (not otherwise wired into any
// test/build step), so CI's `pnpm test` would never catch copy/link drift
// against landing/index.html on its own. This test spawns it directly and
// asserts a clean exit, closing that gap without touching CI workflows.
describe("copy parity", () => {
  it("check-parity.mjs exits 0 (no drift vs landing/index.html)", () => {
    const here = dirname(fileURLToPath(import.meta.url));
    const script = join(here, "../../scripts/check-parity.mjs");

    expect(() => execFileSync("node", [script], { stdio: "pipe" })).not.toThrow();
  });
});
