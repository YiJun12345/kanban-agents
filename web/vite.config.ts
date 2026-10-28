import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    // Forward /graphql to the Rust backend so the browser sees one origin (no CORS)
    proxy: { '/graphql': 'http://localhost:3000' },
  },
})
