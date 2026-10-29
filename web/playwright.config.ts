import { defineConfig } from '@playwright/test'

// The e2e tests start their OWN backend (port 3917, database kanban_e2e) and Vite (port 5917),
// so they never touch your dev servers or your real board.
const BACKEND_PORT = 3917
const WEB_PORT = 5917
const E2E_DATABASE_URL = 'postgres://kanban:kanban@localhost:5432/kanban_e2e'

export const BACKEND_URL = `http://localhost:${BACKEND_PORT}`

export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  workers: 1, // tests share one board, so run them one at a time
  use: { baseURL: `http://localhost:${WEB_PORT}`, trace: 'retain-on-failure' },
  webServer: [
    {
      // Wipe and re-create the e2e database, then start the backend on it
      command: 'sqlx database reset -y && cargo run',
      cwd: '../backend',
      env: { DATABASE_URL: E2E_DATABASE_URL, PORT: String(BACKEND_PORT) },
      url: `${BACKEND_URL}/graphql`,
      timeout: 180_000, // the first cargo build can be slow
      reuseExistingServer: false,
    },
    {
      command: `npx vite --port ${WEB_PORT} --strictPort`,
      env: { BACKEND_URL },
      url: `http://localhost:${WEB_PORT}`,
      reuseExistingServer: false,
    },
  ],
})
