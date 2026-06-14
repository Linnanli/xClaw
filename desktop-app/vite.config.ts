import { resolve } from 'path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [tailwindcss(), react()],
  resolve: {
    alias: {
      '@': resolve('src/renderer/src'),
      '@renderer': resolve('src/renderer/src')
    }
  }
})
