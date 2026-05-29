// Augments Vitest's `expect` with @testing-library/jest-dom matchers
// (toBeInTheDocument, toHaveFocus, toBeDisabled, …) so test files type-check.
// The runtime registration lives in ../vitest.setup.ts; this file only makes
// the matcher types visible to the TypeScript program.
import '@testing-library/jest-dom/vitest';
