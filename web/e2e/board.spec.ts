import { expect, test, type Page } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { BACKEND_URL } from '../playwright.config'

let root: string

// Before any test: a throwaway git repo and a project pointing at it
test.beforeAll(async () => {
  root = mkdtempSync(join(tmpdir(), 'kanban-e2e-'))
  const repo = join(root, 'repo')
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

async function addTask(page: Page, title: string, prompt: string) {
  await page.getByPlaceholder('Task title').fill(title)
  await page.getByPlaceholder('Prompt for the agent').fill(prompt)
  await page.getByRole('button', { name: 'Add task' }).click()
  await expect(column(page, 'To Do').locator('.card', { hasText: title })).toBeVisible()
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

// --- tests ---

test('agent works on a task and the review shows its diff', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'E2E board' })).toBeVisible()

  await addTask(page, 'Add greeting', 'add a greeting')
  await card(page, 'Add greeting').click() // open the log panel
  await drag(page, 'Add greeting', 'In Progress')

  // Live logs arrive through the subscription while the agent runs
  const panel = page.locator('.log-panel')
  await expect(panel.locator('pre').first()).toContainText('working... step 1/3')

  // The card moves by itself when the agent finishes
  await expect(column(page, 'In Review').locator('.card', { hasText: 'Add greeting' })).toBeVisible({
    timeout: 15_000,
  })
  await expect(panel.locator('pre').first()).toContainText('committed changes')
  await expect(panel.getByRole('heading', { name: 'Changes' })).toBeVisible()
  await expect(panel.locator('pre.diff .add', { hasText: '+- add a greeting' })).toBeVisible()
})

test('failing agent moves the card to Failed', async ({ page }) => {
  await page.goto('/')
  await addTask(page, 'Broken task', 'please fail')
  await drag(page, 'Broken task', 'In Progress')

  await expect(column(page, 'Failed').locator('.card', { hasText: 'Broken task' })).toBeVisible({
    timeout: 15_000,
  })
  await card(page, 'Broken task').click()
  await expect(page.locator('.log-panel pre').first()).toContainText('something went wrong')
  await expect(page.locator('.log-panel pre.diff')).toHaveCount(0) // nothing to review
})

test('board rules: cannot skip straight to Done', async ({ page }) => {
  await page.goto('/')
  await addTask(page, 'Skipper', 'x')
  await drag(page, 'Skipper', 'Done')

  await expect(page.locator('p.error')).toHaveText('cannot move task from Todo to Done')
  await expect(column(page, 'To Do').locator('.card', { hasText: 'Skipper' })).toBeVisible()
})
