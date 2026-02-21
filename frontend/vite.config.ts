import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig(({ mode }) => {
  const e2eMode = mode === 'e2e'

  return {
    plugins: [svelte()],
    resolve: {
      alias: {
        '@': fileURLToPath(new URL('./src', import.meta.url)),
        ...(e2eMode
          ? {
              '@tauri-apps/api/core': fileURLToPath(new URL('./src/test/mocks/tauri/core.ts', import.meta.url)),
              '@tauri-apps/api/event': fileURLToPath(new URL('./src/test/mocks/tauri/event.ts', import.meta.url)),
              '@tauri-apps/api/window': fileURLToPath(new URL('./src/test/mocks/tauri/window.ts', import.meta.url)),
              '@tauri-apps/api/path': fileURLToPath(new URL('./src/test/mocks/tauri/path.ts', import.meta.url)),
              '@tauri-apps/api/webview': fileURLToPath(new URL('./src/test/mocks/tauri/webview.ts', import.meta.url)),
            }
          : {}),
      },
    },
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
            svelte: ['svelte', 'svelte/internal'],
            tauri: ['@tauri-apps/api'],
          },
        },
      },
    },
  }
})
