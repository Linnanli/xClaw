import { defineConfig } from 'vite'
import path from 'path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [
    // The React and Tailwind plugins are both required for Make, even if
    // Tailwind is not being actively used – do not remove them
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      // Alias @ to the src directory
      '@': path.resolve(__dirname, './src'),
      // Alias @app to the app directory
      '@app': path.resolve(__dirname, './src/app'),
      // Alias @config to the config directory
      '@config': path.resolve(__dirname, './src/app/config'),
      // Alias @components to the components directory
      '@components': path.resolve(__dirname, './src/app/components'),
      // Alias @hooks to the hooks directory
      '@hooks': path.resolve(__dirname, './src/app/hooks'),
      // Alias @utils to the utils directory
      '@utils': path.resolve(__dirname, './src/app/utils'),
      // Alias @contexts to the contexts directory
      '@contexts': path.resolve(__dirname, './src/app/contexts'),
      // Alias @services to the services directory
      '@services': path.resolve(__dirname, './src/app/services'),
    },
  },
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: false,
  },

  // File types to support raw imports. Never add .css, .tsx, or .ts files to this.
  assetsInclude: ['**/*.svg', '**/*.csv'],
})
