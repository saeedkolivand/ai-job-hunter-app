import { defineConfig } from "vitest/config";

// Unit tests are pure (scene resolver, formatters, svh-freeze math) -- no DOM,
// so the default node environment is correct.
export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
