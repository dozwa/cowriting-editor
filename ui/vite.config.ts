import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, proxy: { '/bridge': 'http://127.0.0.1:1421' } },
  build: { target: 'es2022', chunkSizeWarningLimit: 1500 },
  test: { environment: 'jsdom', setupFiles: ['./src/test-setup.ts'] },
})
