import { useSubscription } from 'urql'
import { graphql } from './gql'
import type { Entry } from './journal'

const JournalSubscription = graphql(`
  subscription JournalLive($taskId: UUID!) {
    journalAdded(taskId: $taskId) {
      id
      seq
      runId
      actor
      kind
      noteKind
      author
      message
      details
      createdAt
    }
  }
`)

/** Saved entries plus the ones that arrive live while the agent works, oldest first */
export function useJournal(taskId: string, saved: Entry[]): Entry[] {
  const [live] = useSubscription(
    { query: JournalSubscription, variables: { taskId } },
    (entries: Entry[] = [], event) => [...entries, event.journalAdded],
  )
  // Saved + live entries can overlap, so merge them by id and sort by seq
  const byId = new Map([...saved, ...(live.data ?? [])].map((e) => [e.id, e]))
  return [...byId.values()].sort((a, b) => a.seq - b.seq)
}
