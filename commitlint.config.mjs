/** @type {import('@commitlint/types').UserConfig} */
export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [
      2,
      'always',
      [
        'feat', // New feature
        'fix', // Bug fix
        'perf', // Performance improvement
        'refactor', // Code refactor (no feature/fix)
        'ui', // UI/UX changes
        'style', // Code style (formatting, whitespace)
        'test', // Tests
        'docs', // Documentation
        'build', // Build system changes
        'ci', // CI/CD changes
        'chore', // Maintenance
        'revert', // Revert a commit
      ],
    ],
    'subject-case': [2, 'always', 'lower-case'],
    'subject-max-length': [2, 'always', 100],
    'body-max-line-length': [2, 'always', 200],
  },
};
