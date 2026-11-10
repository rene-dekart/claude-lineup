import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The dev port must match tauri.conf.json > build > devUrl.
const DEV_PORT = process.env['DEV_PORT'] ?? '5180'

export default defineConfig({
  plugins: [react()],
  server: { port: Number(DEV_PORT) },
  build: { outDir: 'dist/ui', emptyOutDir: true },
})
