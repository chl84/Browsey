import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

export default defineConfig({
  plugins: [svelte()],
  server: {
    host: '0.0.0.0',
    port: 5173,
    strictPort: true,
  },
  preview: {
    port: 5173,
  },
  build: {
    outDir: '../dist',
    emptyOutDir: true,
    chunkSizeWarningLimit: 800,
    rollupOptions: {
      output: {
        manualChunks: {
          pdfjs: ['pdfjs-dist', 'pdfjs-dist/build/pdf.worker.min.mjs'],
          svelte: ['svelte', 'svelte/internal'],
          tauri: ['@tauri-apps/api'],
        },
      },
    },
  },
})
