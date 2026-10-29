import { useMemo } from 'react'
import { useQuery, useSubscription } from 'urql'
import { graphql } from '../gql'

const TaskLogsQuery = graphql(`
  query TaskLogs($id: UUID!) {
    task(id: $id) {
      id
      title
      status
      prompt
      diff
      logs {
        runId
        seq
        content
      }
    }
  }
`)

const TaskLogsSubscription = graphql(`
  subscription TaskLogsLive($taskId: UUID!) {
    taskLogs(taskId: $taskId) {
      runId
      seq
      content
    }
  }
`)

// Colour each line of a unified diff: + added, - removed, @@ position markers
function DiffView({ diff }: { diff: string }) {
    return (
      <pre className="diff">
        {diff.split('\n').map((line, i) => {
          const kind = line.startsWith('+') ? 'add' : line.startsWith('-') ? 'del' : line.startsWith('@@') ? 'hunk' : ''
          return (
            <span key={i} className={kind}>
              {line}
              {'\n'}
            </span>
          )
        })}
      </pre>
    )
  }
  
type Line = { runId: string; seq: number; content: string }

export function LogPanel({ taskId, onClose }: { taskId: string; onClose: () => void }) {
  // Saved logs from the database (refetched when a Task changes, e.g. status)
  const context = useMemo(() => ({ additionalTypenames: ['Task'] }), [])
  const [{ data }] = useQuery({
    query: TaskLogsQuery,
    variables: { id: taskId },
    requestPolicy: 'cache-and-network',
    context,
  })

  // New lines as the agent prints them; the reducer appends each one to a list
  const [live] = useSubscription(
    { query: TaskLogsSubscription, variables: { taskId } },
    (lines: Line[] = [], event) => [...lines, event.taskLogs],
  )

  const task = data?.task
  if (!task) return null

  // Saved + live lines can overlap, so merge them by seq and only show the newest run
  const all = [...task.logs, ...(live.data ?? [])]
  const latestRun = all.at(-1)?.runId
  const bySeq = new Map(all.filter((l) => l.runId === latestRun).map((l) => [l.seq, l]))
  const lines = [...bySeq.values()].sort((a, b) => a.seq - b.seq)

  return (
    <aside className="log-panel">
      <header>
        <h2>{task.title}</h2>
        <button onClick={onClose}>✕</button>
      </header>
      <p className="status">{task.status}</p>
      <p className="prompt">{task.prompt}</p>
      <pre>
        {lines.length === 0
          ? 'No agent output yet. Drag the card to In Progress to start.'
          : lines.map((l) => l.content).join('\n')}
      </pre>

        {task.diff && (
        <>
            <h3>Changes</h3>
            <DiffView diff={task.diff} />
        </>
        )}
        </aside>

  )
}
