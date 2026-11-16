import { expect, test, type Page } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { BACKEND_URL } from '../playwright.config'

let root: string
let repo: string

// Before any test: a throwaway git repo and a project pointing at it
test.beforeAll(async () => {
  root = mkdtempSync(join(tmpdir(), 'kanban-e2e-'))
  repo = join(root, 'repo')
  mkdirSync(repo)
  const git = (...args: string[]) => execFileSync('git', ['-C', repo, ...args])
  git('init', '-b', 'main')
  writeFileSync(join(repo, 'README.md'), '# E2E repo\n')
  git('add', '-A')
  git('-c', 'user.name=test', '-c', 'user.email=test@localhost', 'commit', '-m', 'init')

  const res = await fetch(`${BACKEND_URL}/graphql`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      query: 'mutation($repo: String!) { createProject(name: "E2E board", repoPath: $repo) { id } }',
      variables: { repo },
    }),
  })
  expect((await res.json()).errors).toBeUndefined()
})

test.afterAll(() => rmSync(root, { recursive: true, force: true }))

// --- helpers ---

const column = (page: Page, name: string) =>
  page.locator('.column', { has: page.getByRole('heading', { name, exact: true }) })

const card = (page: Page, title: string) => page.locator('.card', { hasText: title })

const inColumn = (page: Page, name: string, title: string) =>
  column(page, name).locator('.card', { hasText: title })

async function addTask(page: Page, title: string, description: string) {
  await page.getByPlaceholder('Task title').fill(title)
  await page.getByPlaceholder('Description').fill(description)
  await page.getByRole('button', { name: 'Add task' }).click()
  await expect(inColumn(page, 'To Do', title)).toBeVisible()
}

// dnd-kit listens to pointer moves, so drag with small mouse steps like a real user
async function drag(page: Page, title: string, to: string) {
  const from = (await card(page, title).boundingBox())!
  const target = (await column(page, to).boundingBox())!
  await page.mouse.move(from.x + 20, from.y + 10)
  await page.mouse.down()
  await page.mouse.move(from.x + 40, from.y + 20, { steps: 5 }) // past the 5px threshold
  await page.mouse.move(target.x + 60, target.y + 60, { steps: 10 })
  await page.mouse.up()
}

/** Add a task, start its agent by dragging, and wait until it is in In Review */
async function taskInReview(page: Page, title: string, description: string) {
  await addTask(page, title, description)
  await drag(page, title, 'In Progress')
  await expect(inColumn(page, 'In Review', title)).toBeVisible({ timeout: 15_000 })
}

// --- tests ---

test('agent works on a task; the review shows its summary, diff and journal', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'E2E board' })).toBeVisible()

  await addTask(page, 'Add greeting', 'add a greeting')
  await card(page, 'Add greeting').click() // open the task panel
  await drag(page, 'Add greeting', 'In Progress')

  // Live output arrives through the subscription while the agent runs
  const panel = page.locator('.panel')
  await expect(panel.locator('pre.terminal')).toContainText('working... step 1/3')
  await expect(page.locator('.stats')).toContainText('1 agent running')

  // The card moves by itself when the agent finishes
  await expect(inColumn(page, 'In Review', 'Add greeting')).toBeVisible({ timeout: 15_000 })
  await expect(panel.locator('pre.terminal')).toContainText('committed changes')
  await expect(panel.locator('.stamp')).toHaveText('In Review')
  await expect(panel.getByRole('heading', { name: 'Changes' })).toBeVisible()
  await expect(panel.locator('pre.diff .add', { hasText: '+- add a greeting' })).toBeVisible()

  // The agent's summary is the first thing a reviewer reads, and the card shows it too
  await expect(panel.locator('.summary')).toHaveText('Added "add a greeting" to AGENT_NOTES.md')
  await expect(card(page, 'Add greeting').locator('.card-headline')).toContainText(
    'Agent: Added "add a greeting"',
  )

  // Activity: your move, then the run as one group, then the system's move
  await expect(panel.locator('.entry').first()).toContainText('Moved from To Do to In Progress')
  const run = panel.locator('.run')
  await expect(run.locator('summary').first()).toContainText('Run #1')
  await expect(run).toHaveAttribute('data-state', 'finished')
  await expect(run.locator('.entry[data-actor="AGENT"]').first()).toContainText('Plan: add a greeting')
  await expect(run.locator('.entry[data-kind="COMMIT"]')).toContainText('AGENT_NOTES.md')
  await expect(panel.locator('.entry').last()).toContainText('Moved from In Progress to In Review')

  // Filters: Notes shows only notes and summaries
  await panel.getByRole('button', { name: 'Notes' }).click()
  await expect(panel.locator('.entry[data-kind="STATUS"]')).toHaveCount(0)
  await expect(panel.locator('.entry[data-kind="SUMMARY"]')).toBeVisible()
})

test('approving merges the work into main', async ({ page }) => {
  await page.goto('/')
  await taskInReview(page, 'Ship me', 'ship this line')
  expect(existsSync(join(repo, 'AGENT_NOTES.md')), 'main is untouched before approval').toBe(false)

  await card(page, 'Ship me').click()
  const panel = page.locator('.panel')
  await panel.getByRole('button', { name: 'Approve & merge' }).click()

  await expect(inColumn(page, 'Done', 'Ship me')).toBeVisible()
  await expect(panel.locator('.merged')).toContainText('Merged into main as')
  await expect(panel.locator('.entry[data-kind="MERGE"]')).toContainText('into main as')
  // The diff can still be read after the branch is gone
  await expect(panel.locator('pre.diff .add', { hasText: '+- ship this line' })).toBeVisible()

  // The real repository has the agent's file on main, in a merge commit named after the task
  expect(existsSync(join(repo, 'AGENT_NOTES.md'))).toBe(true)
  const log = execFileSync('git', ['-C', repo, 'log', '-1', '--format=%s']).toString()
  expect(log.trim()).toBe('Merge task: Ship me')
})

test('requesting changes sends the task back with your feedback', async ({ page }) => {
  await page.goto('/')
  await taskInReview(page, 'Needs work', 'write a slogan')

  await card(page, 'Needs work').click()
  const panel = page.locator('.panel')
  await panel.getByRole('button', { name: 'Request changes' }).click()
  await panel.getByPlaceholder('What should the agent change?').fill('make it shorter')
  await panel.getByRole('button', { name: 'Send back to agent' }).click()

  // It runs again by itself and comes back to review
  await expect(inColumn(page, 'In Progress', 'Needs work')).toBeVisible()
  await expect(inColumn(page, 'In Review', 'Needs work')).toBeVisible({ timeout: 15_000 })

  // Your feedback is on record, and the second run says it read it
  await expect(panel.locator('.entry[data-feedback]')).toContainText('make it shorter')
  await expect(panel.locator('.entry[data-feedback]')).toContainText('requested changes')
  const runs = panel.locator('.run')
  await expect(runs).toHaveCount(2)
  await expect(runs.nth(1)).toContainText('Addressing review feedback: make it shorter')
  await expect(panel.locator('pre.diff')).toContainText('after review: make it shorter')
})

test('people can add notes to the journal', async ({ page }) => {
  await page.goto('/')
  await addTask(page, 'Noted task', 'x')
  await card(page, 'Noted task').click()

  await page.getByPlaceholder('Add a note…').fill('Remember to update the docs')
  await page.getByRole('button', { name: 'Add note' }).click()

  const note = page.locator('.journal .entry[data-actor="HUMAN"][data-kind="NOTE"]')
  await expect(note).toContainText('Remember to update the docs')
  await expect(note).toContainText('You')
  await expect(page.getByPlaceholder('Add a note…')).toHaveValue('')

  // The card shows the newest note, live
  await expect(card(page, 'Noted task').locator('.card-headline')).toContainText(
    'You: Remember to update the docs',
  )
})

test('a failing agent moves the card to Failed, and can be retried', async ({ page }) => {
  await page.goto('/')
  await addTask(page, 'Broken task', 'please fail')
  await drag(page, 'Broken task', 'In Progress')

  await expect(inColumn(page, 'Failed', 'Broken task')).toBeVisible({ timeout: 15_000 })
  await expect(card(page, 'Broken task').locator('.card-headline')).toContainText(
    'Could not finish: something went wrong',
  )
  await card(page, 'Broken task').click()
  const panel = page.locator('.panel')
  await expect(panel.locator('.run')).toHaveAttribute('data-state', 'failed')
  await panel.getByText('View raw output').click()
  await expect(panel.locator('pre.terminal')).toContainText('something went wrong')
  await expect(panel.locator('pre.diff')).toHaveCount(0) // nothing to review

  // Retry starts run #2 straight from Failed
  await panel.getByRole('button', { name: 'Retry' }).click()
  await expect(inColumn(page, 'In Progress', 'Broken task')).toBeVisible()
  await expect(panel.locator('.run')).toHaveCount(2)
  await expect(inColumn(page, 'Failed', 'Broken task')).toBeVisible({ timeout: 15_000 })
})

test('a running agent can be stopped', async ({ page }) => {
  await page.goto('/')
  await addTask(page, 'Stop me', 'take your time')
  await card(page, 'Stop me').click()
  const panel = page.locator('.panel')
  await panel.getByRole('button', { name: 'Start agent' }).click()
  await expect(panel.locator('pre.terminal')).toContainText('working... step 1/3')

  await panel.getByRole('button', { name: 'Stop agent' }).click()

  await expect(inColumn(page, 'To Do', 'Stop me')).toBeVisible()
  await expect(panel.locator('.run')).toHaveAttribute('data-state', 'stopped')
  await expect(panel.locator('.entry[data-kind="RUN_FINISHED"]')).toContainText('Run stopped by you')
  // It really stopped: the card is still in To Do after the agent would have finished
  await page.waitForTimeout(3500)
  await expect(inColumn(page, 'To Do', 'Stop me')).toBeVisible()
})

test('board rules: cannot skip straight to Done', async ({ page }) => {
  await page.goto('/')
  await addTask(page, 'Skipper', 'x')
  await drag(page, 'Skipper', 'Done')

  await expect(page.locator('.toast .error')).toHaveText('cannot move task from Todo to Done')
  await expect(inColumn(page, 'To Do', 'Skipper')).toBeVisible()
})

test('the logbook lists the whole project and opens tasks', async ({ page }) => {
  await page.goto('/')
  await addTask(page, 'Logged task', 'x')
  await card(page, 'Logged task').click()
  await page.getByPlaceholder('Add a note…').fill('First line in the logbook')
  await page.getByRole('button', { name: 'Add note' }).click()
  await page.locator('.panel').getByRole('button', { name: 'Close' }).click()

  await page.getByRole('navigation', { name: 'View' }).getByRole('button', { name: 'Logbook' }).click()
  const newest = page.locator('.log-row').first()
  await expect(newest).toContainText('First line in the logbook')
  await expect(newest).toContainText('Logged task')
  await expect(page.locator('.logbook-day h3').first()).toHaveText('Today')

  // New entries appear without reloading
  await newest.click()
  const panel = page.locator('.panel')
  await expect(panel.getByRole('heading', { name: 'Logged task' })).toBeVisible()
  await panel.getByPlaceholder('Add a note…').fill('Second line')
  await panel.getByRole('button', { name: 'Add note' }).click()
  await expect(page.locator('.log-row').first()).toContainText('Second line')
})

test('a project is created from the board, and only for a real git repo', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'New project' }).click()
  const dialog = page.getByRole('dialog', { name: 'New project' })
  await dialog.getByLabel('Project name').fill('Second board')

  await dialog.getByLabel('Git repository on this computer').fill('/nonexistent/repo')
  await dialog.getByRole('button', { name: 'Create project' }).click()
  await expect(dialog.getByRole('alert')).toContainText('is not a git repository')

  await dialog.getByLabel('Git repository on this computer').fill(repo)
  await dialog.getByRole('button', { name: 'Create project' }).click()

  // The board switches to the new, empty project and remembers it
  await expect(page.getByRole('heading', { name: 'Second board' })).toBeVisible()
  await expect(page.locator('.card')).toHaveCount(0)
  await page.reload()
  await expect(page.getByRole('heading', { name: 'Second board' })).toBeVisible()
})

test('agents that cannot run here cannot be picked', async ({ page }) => {
  await page.goto('/')
  const claude = page.getByLabel('Agent').locator('option', { hasText: 'Claude Code' })
  await expect(claude).toBeDisabled()
  await expect(claude).toContainText('KANBAN_ANTHROPIC_API_KEY is not set')
  await expect(page.getByLabel('Agent')).toHaveValue('FAKE')
})
