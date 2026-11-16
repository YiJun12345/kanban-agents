import type { IconName } from './components/Icon'
import type { JournalActor, JournalKind, NoteKind } from './gql/graphql'

/** One line of a task's journal, as the API returns it */
export type Entry = {
  id: string
  seq: number
  runId?: string | null
  actor: JournalActor
  kind: JournalKind
  noteKind?: NoteKind | null
  author?: string | null // the worker that signed the entry
  message: string
  details: Record<string, unknown>
  createdAt: string
}

export const WHO: Record<JournalActor, string> = {
  HUMAN: 'You',
  AGENT: 'Agent',
  SYSTEM: 'System',
}

export const KIND_ICON: Record<JournalKind, IconName> = {
  STATUS: 'arrow',
  RUN_STARTED: 'play',
  RUN_FINISHED: 'stop',
  NOTE: 'pen',
  SUMMARY: 'star',
  COMMIT: 'commit',
  MERGE: 'merge',
  ERROR: 'alert',
  CHECKPOINT: 'flag',
}

/** Who to show for an entry: an outside worker by its name, anyone else by role */
export function who(entry: { actor: JournalActor; author?: string | null }) {
  const named = entry.author && !entry.author.startsWith('builtin-')
  return named ? entry.author! : WHO[entry.actor]
}

// What the agent itself wrote, as opposed to what the system recorded about it
const WRITTEN: JournalKind[] = ['NOTE', 'SUMMARY', 'CHECKPOINT']

// Notes are what people and agents wrote; history is what the system recorded
export const FILTERS = {
  All: () => true,
  Notes: (e: Entry) => WRITTEN.includes(e.kind),
  History: (e: Entry) => !WRITTEN.includes(e.kind),
}
export type Filter = keyof typeof FILTERS

/** "#3F2A": a short ticket number made from the task's id */
export const ticket = (id: string) => `#${id.slice(0, 4).toUpperCase()}`

/** "14:05" */
export const clock = (iso: string) =>
  new Date(iso).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false })

/** "Today", "Yesterday" or "14 November" */
export function dayLabel(iso: string) {
  const day = new Date(iso)
  const today = new Date()
  const yesterday = new Date()
  yesterday.setDate(today.getDate() - 1)
  if (day.toDateString() === today.toDateString()) return 'Today'
  if (day.toDateString() === yesterday.toDateString()) return 'Yesterday'
  return day.toLocaleDateString([], { day: 'numeric', month: 'long' })
}
