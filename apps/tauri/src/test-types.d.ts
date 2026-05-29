// Augments Vitest's `expect` with @testing-library/jest-dom matchers
// (toBeInTheDocument, toHaveFocus, toBeDisabled, …) so renderer test files
// type-check. Runtime registration lives in ../../vitest.setup.ts.
import '@testing-library/jest-dom/vitest';
