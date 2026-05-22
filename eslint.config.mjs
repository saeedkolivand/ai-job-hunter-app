// @ts-check
import eslint from '@eslint/js';
import tseslint from 'typescript-eslint';
import reactPlugin from 'eslint-plugin-react';
import reactHooksPlugin from 'eslint-plugin-react-hooks';
import unusedImports from 'eslint-plugin-unused-imports';
import simpleImportSort from 'eslint-plugin-simple-import-sort';
import storybookPlugin from 'eslint-plugin-storybook';
import globals from 'globals';

// ── AST selectors ─────────────────────────────────────────────────────────────

const HARDCODED_HEX_IN_CLASSNAME =
  'JSXAttribute[name.name="className"] Literal[value=/\\[#[0-9a-fA-F]{3,6}/]';

const HARDCODED_HEX_IN_STYLE =
  'JSXAttribute[name.name="style"] Property > Literal[value=/#[0-9a-fA-F]{3,6}/]';

const INLINE_TRANSITION_OBJECT =
  'JSXAttribute[name.name="transition"] > JSXExpressionContainer > ObjectExpression[properties.length>0]';

const WINDOW_API_DIRECT =
  'MemberExpression[object.type="MemberExpression"][object.object.name="window"][object.property.name="api"]';

// Raw interactive HTML — must use shared primitives from @ajh/ui
const RAW_BUTTON = 'JSXOpeningElement[name.name="button"]';
const RAW_SELECT = 'JSXOpeningElement[name.name="select"]';
const RAW_TEXTAREA = 'JSXOpeningElement[name.name="textarea"]';

// ── Forbidden deep UI import paths (all resolve to @ajh/ui exports) ──────────
// UpdateBanner is intentionally omitted — it is app-specific and lives only
// in apps/desktop/src/renderer/components/ui/UpdateBanner.tsx.
const DEEP_UI_IMPORTS = [
  '@/components/ui/ActionTile',
  '@/components/ui/Button',
  '@/components/ui/CardSkeleton',
  '@/components/ui/ConfirmModal',
  '@/components/ui/EmptyState',
  '@/components/ui/ErrorBoundary',
  '@/components/ui/ErrorState',
  '@/components/ui/GlassCard',
  '@/components/ui/GlassOverlay',
  '@/components/ui/IconBadge',
  '@/components/ui/IconText',
  '@/components/ui/Input',
  '@/components/ui/LoadingSkeleton',
  '@/components/ui/MarkdownMessage',
  '@/components/ui/ModalShell',
  '@/components/ui/OptionTile',
  '@/components/ui/RowSkeleton',
  '@/components/ui/SectionHeader',
  '@/components/ui/SectionLabel',
  '@/components/ui/SelectDropdown',
  '@/components/ui/SettingsSection',
  '@/components/ui/Skeleton',
  '@/components/ui/StreamingText',
  '@/components/ui/TextArea',
  '@/components/ui/Toast',
  // barrel re-export is also banned — import from '@ajh/ui' directly
  // Note: '@/components/ui/UpdateBanner' is intentionally excluded (app-specific component)
  '@/components/ui/index',
];

// ── Renderer package boundary — main-process packages ────────────────────────
const MAIN_PROCESS_PACKAGES = [
  '@ajh/core',
  '@ajh/core/**',
  '@ajh/ai',
  '@ajh/ai/**',
  '@ajh/data',
  '@ajh/data/**',
  '@ajh/workers',
  '@ajh/workers/**',
];

// ── Shared no-restricted-imports sets ────────────────────────────────────────

const I18N_IMPORT_RESTRICTION = {
  paths: [
    {
      name: 'react-i18next',
      importNames: ['useTranslation'],
      message:
        "Import useTranslation from '@/lib/i18n' (the adapter), not directly from 'react-i18next'.",
    },
    {
      name: 'i18next',
      importNames: ['default'],
      message:
        "Import the i18n instance from '@/lib/i18n' or '@/i18n', not directly from 'i18next'.",
    },
  ],
};

export default tseslint.config(
  // ── Global linter options ───────────────────────────────────────────────────
  // noInlineConfig bans all inline eslint-disable/enable comments.
  // Exceptions must live here as config-level overrides — visible and reviewable.
  {
    linterOptions: {
      noInlineConfig: true,
      reportUnusedDisableDirectives: 'error',
    },
  },

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
      'unused-imports': unusedImports,
      'simple-import-sort': simpleImportSort,
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
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-unused-vars': 'off', // replaced by unused-imports/no-unused-vars
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

      // ── Unused imports — auto-fixable ───────────────────────────────────────
      'unused-imports/no-unused-imports': 'error',
      'unused-imports/no-unused-vars': [
        'warn',
        {
          vars: 'all',
          varsIgnorePattern: '^_',
          args: 'after-used',
          argsIgnorePattern: '^_',
        },
      ],

      // ── Import ordering — auto-fixable ──────────────────────────────────────
      // Groups: 1) Node built-ins  2) External packages  3) @ajh/* packages
      //         4) @/ aliases      5) Relative imports
      'simple-import-sort/imports': [
        'error',
        {
          groups: [['^node:'], ['^(?!@ajh|@/)\\w', '^@(?!ajh|/)'], ['^@ajh/'], ['^@/'], ['^\\.']],
        },
      ],
      'simple-import-sort/exports': 'error',
      'no-duplicate-imports': 'error',

      // ── General quality ─────────────────────────────────────────────────────
      'no-console': ['warn', { allow: ['warn', 'error'] }],
      'no-debugger': 'error',
      'prefer-const': 'error',
      'no-var': 'error',
      'object-shorthand': 'error',

      // ── Design system: no hardcoded hex colors ──────────────────────────────
      'no-restricted-syntax': [
        'warn',
        {
          selector: HARDCODED_HEX_IN_CLASSNAME,
          message:
            'Use brand tokens (text-brand, text-brand-soft, bg-brand, border-brand) instead of hardcoded hex in className.',
        },
        {
          selector: HARDCODED_HEX_IN_STYLE,
          message:
            'Use CSS custom properties (var(--color-brand), var(--color-brand-soft)) instead of hardcoded hex in style objects.',
        },
      ],
    },
  },

  // ── All renderer source — i18n adapter + package boundary ──────────────────
  {
    files: ['apps/desktop/src/renderer/**/*.ts', 'apps/desktop/src/renderer/**/*.tsx'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          ...I18N_IMPORT_RESTRICTION,
          patterns: [
            {
              group: MAIN_PROCESS_PACKAGES,
              message:
                'Main-process packages (@ajh/core, @ajh/ai, @ajh/data, @ajh/workers) are not available in the renderer. Add an IPC method instead.',
            },
          ],
        },
      ],
    },
  },

  // ── UI layer: features, routes, shared components ───────────────────────────
  // These files have the full set of design system + architecture restrictions.
  {
    files: [
      'apps/desktop/src/renderer/features/**/*.tsx',
      'apps/desktop/src/renderer/routes/**/*.tsx',
      'apps/desktop/src/renderer/components/**/*.tsx',
    ],
    rules: {
      // Extends the renderer-level restriction with deep UI import ban
      'no-restricted-imports': [
        'error',
        {
          ...I18N_IMPORT_RESTRICTION,
          patterns: [
            {
              group: MAIN_PROCESS_PACKAGES,
              message:
                'Main-process packages are not available in the renderer. Add an IPC method instead.',
            },
            {
              group: DEEP_UI_IMPORTS,
              message:
                "Import from '@ajh/ui' directly instead of deep component paths. Example: import { Button } from '@ajh/ui'. The only exception is UpdateBanner: import { UpdateBanner } from '@/components/ui/UpdateBanner'.",
            },
          ],
        },
      ],
      'no-restricted-syntax': [
        'error',
        {
          selector: HARDCODED_HEX_IN_CLASSNAME,
          message:
            'Use brand tokens (text-brand, text-brand-soft, bg-brand, border-brand) instead of hardcoded hex.',
        },
        {
          selector: HARDCODED_HEX_IN_STYLE,
          message:
            'Use CSS custom properties (var(--color-brand)) instead of hardcoded hex in style objects.',
        },
        {
          selector: INLINE_TRANSITION_OBJECT,
          message:
            "Use motion tokens from '@/lib/motion': import { transition } from '@/lib/motion'; then use transition.normal, transition.fast, transition.spring, etc.",
        },
        {
          selector: WINDOW_API_DIRECT,
          message:
            "Don't call window.api.* directly in UI files. Use service hooks from '@/services/' instead (e.g. useDocuments, useJobs). This enforces the Ports & Adapters boundary.",
        },
        {
          selector: RAW_BUTTON,
          message:
            "Use <Button> from '@ajh/ui' instead of raw <button>. It handles focus, disabled states, and design tokens correctly.",
        },
        {
          selector: RAW_SELECT,
          message: "Use <SelectDropdown> from '@ajh/ui' instead of raw <select>.",
        },
        {
          selector: RAW_TEXTAREA,
          message: "Use <TextArea> from '@ajh/ui' instead of raw <textarea>.",
        },
      ],
    },
  },

  // ── Type-checked rules (renderer only) ─────────────────────────────────────
  // Requires TypeScript project references — adds ~3-5x linting overhead.
  // To disable: comment out this entire block. Rules here catch async misuse
  // that plain type checking won't surface until runtime.
  {
    files: ['apps/desktop/src/renderer/**/*.ts', 'apps/desktop/src/renderer/**/*.tsx'],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      '@typescript-eslint/no-floating-promises': 'error',
      '@typescript-eslint/no-misused-promises': [
        'error',
        { checksVoidReturn: { attributes: false } },
      ],
      '@typescript-eslint/no-unnecessary-type-assertion': 'error',
    },
  },

  // ── Service layer — allowed to call window.api directly ───────────────────
  {
    files: ['apps/desktop/src/renderer/services/**/*.ts'],
    rules: {
      'no-restricted-syntax': 'off',
    },
  },

  // ── i18n adapter and setup — allowed to import react-i18next directly ──────
  {
    files: ['apps/desktop/src/renderer/lib/i18n.ts', 'apps/desktop/src/renderer/i18n/index.ts'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },

  // ── Config / build scripts — relax all restrictions ────────────────────────
  {
    files: ['*.config.*', 'scripts/**', '**/*.config.ts'],
    rules: {
      'no-console': 'off',
      'no-undef': 'off',
      '@typescript-eslint/no-explicit-any': 'off',
      '@typescript-eslint/no-require-imports': 'off',
      'no-restricted-imports': 'off',
      'no-restricted-syntax': 'off',
      'simple-import-sort/imports': 'off',
    },
  },

  // ── Storybook story files ─────────────────────────────────────────────────
  {
    files: ['**/*.stories.@(ts|tsx)'],
    plugins: { storybook: storybookPlugin },
    rules: {
      ...Object.fromEntries(
        Object.entries(storybookPlugin.rules ?? {}).map(([k]) => [`storybook/${k}`, 'warn'])
      ),
      // Stories intentionally use raw <button> for demo content
      'no-restricted-syntax': 'off',
    },
  },

  // ── Approved `any` exceptions — complex dynamic ESM types ──────────────────
  // These files use dynamic import() with external libraries whose types are
  // only resolvable at runtime. Inline suppression is banned; this is the
  // approved exception point.
  {
    files: [
      'apps/desktop/src/main/updater.ts',
      'apps/desktop/src/renderer/lib/generate-ai.ts',
      'packages/data/src/files/pdf-adapter.ts',
    ],
    rules: {
      '@typescript-eslint/no-explicit-any': 'off',
    },
  }
);
