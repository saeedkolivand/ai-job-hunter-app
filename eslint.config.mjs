// @ts-check
import eslint from '@eslint/js';
import tseslint from 'typescript-eslint';
import reactPlugin from 'eslint-plugin-react';
import reactHooksPlugin from 'eslint-plugin-react-hooks';
import globals from 'globals';

// ── Patterns that indicate design system bypasses ────────────────────────────
// These selectors are used in no-restricted-syntax rules to prevent developers
// from accidentally bypassing shared primitives or hardcoding visual values.

const HARDCODED_HEX_IN_CLASSNAME =
  'JSXAttribute[name.name="className"] Literal[value=/\\[#[0-9a-fA-F]{3,6}/]';

const HARDCODED_HEX_IN_STYLE =
  'JSXAttribute[name.name="style"] Property > Literal[value=/#[0-9a-fA-F]{3,6}/]';

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.recommended,

  // ── Global ignores ──────────────────────────────────────────────────────────
  {
    ignores: [
      '**/dist/**',
      '**/out/**',
      '**/node_modules/**',
      '**/*.gen.ts',
      '**/*.tsbuildinfo',
      'coverage/**',
    ],
  },

  // ── All TypeScript/TSX source files ────────────────────────────────────────
  {
    files: ['**/*.ts', '**/*.tsx'],
    plugins: {
      react: reactPlugin,
      'react-hooks': reactHooksPlugin,
    },
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    settings: {
      react: { version: 'detect' },
    },
    rules: {
      // ── TypeScript ──────────────────────────────────────────────────────────
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],
      '@typescript-eslint/consistent-type-imports': [
        'error',
        { prefer: 'type-imports', fixStyle: 'inline-type-imports' },
      ],
      '@typescript-eslint/no-import-type-side-effects': 'error',
      '@typescript-eslint/no-non-null-assertion': 'warn',

      // ── React ───────────────────────────────────────────────────────────────
      'react/react-in-jsx-scope': 'off',
      'react/prop-types': 'off',
      'react/display-name': 'off',
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',

      // ── General quality ─────────────────────────────────────────────────────
      'no-console': ['warn', { allow: ['warn', 'error'] }],
      'no-debugger': 'error',
      'prefer-const': 'error',
      'no-var': 'error',
      'object-shorthand': 'error',
      'no-duplicate-imports': 'error',

      // ── i18n adapter enforcement ────────────────────────────────────────────
      // Components must import useTranslation from @/lib/i18n (the adapter), not
      // directly from react-i18next. This isolates the library dependency to one
      // file so a future API change or library swap only requires updating lib/i18n.ts.
      //
      // Allowed exceptions (use eslint-disable-next-line):
      //   - apps/desktop/src/renderer/lib/i18n.ts  (the adapter itself)
      //   - apps/desktop/src/renderer/i18n/index.ts (library setup — uses initReactI18next)
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: 'react-i18next',
              importNames: ['useTranslation'],
              message:
                "Import useTranslation from '@/lib/i18n' instead of 'react-i18next' directly.",
            },
            {
              name: 'i18next',
              importNames: ['default'],
              message:
                "Import the i18n instance from '@/lib/i18n' or '@/i18n' instead of 'i18next' directly.",
            },
          ],
        },
      ],

      // ── Design system enforcement ────────────────────────────────────────────
      // Prevent hardcoded brand hex colors — use text-brand, text-brand-soft, bg-brand etc.
      'no-restricted-syntax': [
        'warn',
        {
          selector: HARDCODED_HEX_IN_CLASSNAME,
          message:
            'Avoid hardcoded hex colors in className. Use brand tokens: text-brand, text-brand-soft, bg-brand, border-brand, etc.',
        },
        {
          selector: HARDCODED_HEX_IN_STYLE,
          message:
            'Avoid hardcoded hex colors in style objects. Use CSS custom properties: var(--color-brand), var(--color-brand-soft), etc.',
        },
      ],
    },
  },

  // ── Renderer-specific rules (feature + route files) ────────────────────────
  {
    files: [
      'apps/desktop/src/renderer/features/**/*.tsx',
      'apps/desktop/src/renderer/routes/**/*.tsx',
      'apps/desktop/src/renderer/components/**/*.tsx',
    ],
    rules: {
      'no-restricted-syntax': [
        'warn',
        {
          selector: HARDCODED_HEX_IN_CLASSNAME,
          message:
            'Avoid hardcoded hex colors in className. Use brand tokens: text-brand, text-brand-soft, bg-brand, border-brand, etc.',
        },
        {
          // Matches: transition={{ duration: ... }} or transition={{ ease: ... }}
          selector:
            'JSXAttribute[name.name="transition"] > JSXExpressionContainer > ObjectExpression[properties.length>0]',
          message:
            'Use motion tokens instead of inline transition objects. Import { transition } from "@/lib/motion" and use transition.normal, transition.fast, transition.relaxed, etc.',
        },
        {
          // Catches window.api.* calls in UI layer — must go through service hooks in @/services/
          selector:
            'MemberExpression[object.type="MemberExpression"][object.object.name="window"][object.property.name="api"]',
          message:
            'Do not call window.api.* directly in UI files. Use service hooks from "@/services/" instead (e.g. useDocuments, useJobs, useAIModels). This enforces the Ports & Adapters boundary.',
        },
      ],
    },
  },

  // ── Service layer — allowed to call window.api directly ───────────────────
  {
    files: ['apps/desktop/src/renderer/services/**/*.ts'],
    rules: {
      'no-restricted-syntax': 'off', // services ARE the port — they own window.api calls
    },
  },

  // ── i18n adapter and setup files — allowed to import react-i18next directly ─
  {
    files: [
      'apps/desktop/src/renderer/lib/i18n.ts', // the adapter — this IS the wrapper
      'apps/desktop/src/renderer/i18n/index.ts', // library init — uses initReactI18next
    ],
    rules: {
      'no-restricted-imports': 'off',
    },
  },

  // ── Relax rules for config/build files ─────────────────────────────────────
  {
    files: ['*.config.*', 'scripts/**', '**/*.config.ts'],
    rules: {
      'no-console': 'off',
      '@typescript-eslint/no-explicit-any': 'off',
      'no-restricted-imports': 'off',
      'no-restricted-syntax': 'off',
    },
  }
);
