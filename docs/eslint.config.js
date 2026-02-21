import js from '@eslint/js'
import svelteParser from 'svelte-eslint-parser'
import tseslint from 'typescript-eslint'
import svelte from 'eslint-plugin-svelte'

const baseRules = {
  // TypeScript + svelte-check already validate symbols/types.
  'no-undef': 'off',
  'no-unused-vars': 'off',
  '@typescript-eslint/no-unused-vars': [
    'error',
    {
      argsIgnorePattern: '^_',
      varsIgnorePattern: '^_',
      caughtErrorsIgnorePattern: '^_',
    },
  ],
}

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
    rules: baseRules,
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
    rules: baseRules,
  },
]
