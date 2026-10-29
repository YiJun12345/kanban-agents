import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    // Forward /graphql to the Rust backend so the browser sees one origin (no CORS)
    // BACKEND_URL lets the e2e tests point at their own backend
    proxy: {
      '/graphql': { target: process.env.BACKEND_URL ?? 'http://localhost:3000', ws: true },
    },
  },
})
