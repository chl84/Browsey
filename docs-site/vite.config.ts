import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

const base = process.env.PAGES_BASE_PATH ?? '/'

export default defineConfig({
  base,
  plugins: [svelte()],
  server: {
    host: '0.0.0.0',
    port: 4173,
    strictPort: true,
  },
  preview: {
    port: 4173,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    chunkSizeWarningLimit: 600,
  },
})
