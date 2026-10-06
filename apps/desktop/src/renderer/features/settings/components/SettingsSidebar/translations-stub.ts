// Kept apart from ./test-support: the `vi.mock` factory loads this lazily while the
// subject (imported by test-support) is itself loading — sharing a module would deadlock.

/** `vi.mock('@ajh/translations')` factory body. */
export const translationsMock = {
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) => {
      // Return the key, appending params as a JSON suffix so tests can assert
      // on the key + presence of injected values (e.g. {count}).
      if (params) return `${key}:${JSON.stringify(params)}`;
      return key;
    },
  }),
};
