import tsParser from '@typescript-eslint/parser'
import svelteParser from 'svelte-eslint-parser'

const features = ['explorer', 'settings', 'network', 'shortcuts']

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
  {
    ignores: ['node_modules/**', 'dist/**', '.svelte-kit/**', 'coverage/**'],
  },
  {
    files: ['src/**/*.{ts,js}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
    },
    rules: enforceDeepFeatureImports,
  },
  {
    files: ['src/**/*.svelte'],
    languageOptions: {
      parser: svelteParser,
      parserOptions: {
        parser: tsParser,
        ecmaVersion: 'latest',
        sourceType: 'module',
        extraFileExtensions: ['.svelte'],
      },
    },
    rules: enforceDeepFeatureImports,
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
