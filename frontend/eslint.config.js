import js from '@eslint/js'
import svelteParser from 'svelte-eslint-parser'
import tseslint from 'typescript-eslint'
import svelte from 'eslint-plugin-svelte'

const features = ['explorer', 'settings', 'network', 'shortcuts']

const pragmaticRuleTuning = {
  // TypeScript already covers undefined symbols through type-checking.
  'no-undef': 'off',
  'no-unused-vars': 'off',
  '@typescript-eslint/no-unused-vars': [
    'warn',
    {
      argsIgnorePattern: '^_',
      varsIgnorePattern: '^_',
      caughtErrorsIgnorePattern: '^_',
    },
  ],
  '@typescript-eslint/no-explicit-any': 'off',
  '@typescript-eslint/no-unused-expressions': 'off',
  // Keep these visible without blocking adoption.
  'no-useless-escape': 'warn',
  'no-unsafe-finally': 'warn',
  // Svelte recommended rules that are too noisy for current code style.
  'svelte/require-each-key': 'off',
  'svelte/infinite-reactive-loop': 'off',
  'svelte/prefer-svelte-reactivity': 'off',
  'svelte/no-immutable-reactive-statements': 'off',
  'svelte/no-useless-mustaches': 'off',
}

const enforceDeepFeatureImports = {
  'no-restricted-imports': [
    'error',
    {
      patterns: [
        {
          regex: '^@/features/[^/]+/.+',
          message: 'Use feature barrel imports (`@/features/<feature>`) across feature boundaries.',
        },
      ],
    },
  ],
}

const enforceCrossFeatureDeepImports = (feature) => ({
  'no-restricted-imports': [
    'error',
    {
      patterns: [
        {
          regex: `^@/features/(?!${feature}(?:$|/))[^/]+/.+`,
          message: 'Cross-feature deep imports are private. Use `@/features/<feature>`.',
        },
      ],
    },
  ],
})

const filesForTsJs = (pattern) => `${pattern}/**/*.{ts,js}`
const filesForSvelte = (pattern) => `${pattern}/**/*.svelte`

export default [
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    ignores: ['node_modules/**', 'dist/**', '.svelte-kit/**', 'coverage/**'],
  },
  {
    files: ['src/**/*.{ts,js}'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
    },
    rules: {
      ...pragmaticRuleTuning,
      ...enforceDeepFeatureImports,
    },
  },
  {
    files: ['src/**/*.svelte'],
    languageOptions: {
      parser: svelteParser,
      parserOptions: {
        parser: tseslint.parser,
        ecmaVersion: 'latest',
        sourceType: 'module',
        extraFileExtensions: ['.svelte'],
      },
    },
    rules: {
      ...pragmaticRuleTuning,
      ...enforceDeepFeatureImports,
    },
  },
  ...features.flatMap((feature) => [
    {
      files: [filesForTsJs(`src/features/${feature}`)],
      rules: enforceCrossFeatureDeepImports(feature),
    },
    {
      files: [filesForSvelte(`src/features/${feature}`)],
      rules: enforceCrossFeatureDeepImports(feature),
    },
  ]),
]
