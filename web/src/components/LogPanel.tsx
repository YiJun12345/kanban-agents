import { useMemo } from 'react'
import { useQuery, useSubscription } from 'urql'
import { graphql } from '../gql'
import { timeAgo } from '../time'

const TaskLogsQuery = graphql(`
  query TaskLogs($id: UUID!) {
    task(id: $id) {
      id
      title
      status
      prompt
      agent
      branch
      updatedAt
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

// Colour each line of a unified diff: + added, - removed, @@ position, file headers muted
function lineKind(line: string) {
  if (line.startsWith('+++') || line.startsWith('---')) return 'meta'
  if (line.startsWith('diff ') || line.startsWith('index ') || line.startsWith('new file')) return 'meta'
  if (line.startsWith('+')) return 'add'
  if (line.startsWith('-')) return 'del'
  if (line.startsWith('@@')) return 'hunk'
  return ''
}

function DiffView({ diff }: { diff: string }) {
  return (
    <pre className="diff">
      {diff.trimEnd().split('\n').map((line, i) => (
        <span key={i} className={lineKind(line)}>
          {line}
          {'\n'}
        </span>
      ))}
    </pre>
  )
}

const STATUS_LABEL: Record<string, string> = {
  TODO: 'To Do',
  IN_PROGRESS: 'In Progress',
  IN_REVIEW: 'In Review',
  DONE: 'Done',
  FAILED: 'Failed',
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

  const filesChanged = task.diff?.match(/^diff --git/gm)?.length ?? 0

  return (
    <aside className="log-panel">
      <header>
        <div>
          <h2>{task.title}</h2>
          <span className="status-badge" data-status={task.status}>
            {STATUS_LABEL[task.status]}
          </span>
        </div>
        <button onClick={onClose} aria-label="Close">
          ✕
        </button>
      </header>

      <dl className="details">
        <dt>Assignee</dt>
        <dd className="assignee">
          <span className="avatar">AI</span> {task.agent.toLowerCase()} agent
        </dd>
        <dt>Branch</dt>
        <dd>{task.branch ? <code>{task.branch}</code> : 'not started'}</dd>
        <dt>Updated</dt>
        <dd>{timeAgo(task.updatedAt)}</dd>
      </dl>

      <section>
        <h3>Description</h3>
        <p className="description">{task.prompt || 'No description.'}</p>
      </section>

      {task.diff && (
        <section>
          <h3>
            Changes
            <span className="aside">
              {filesChanged} file{filesChanged === 1 ? '' : 's'}
            </span>
          </h3>
          <DiffView diff={task.diff} />
        </section>
      )}

      <section>
        <h3>
          Activity
          {task.status === 'IN_PROGRESS' && (
            <span className="live">
              <span className="pulse" /> live
            </span>
          )}
        </h3>
        <pre className="terminal">
          {lines.length === 0
            ? 'No activity yet. Drag the card to In Progress to start the agent.'
            : lines.map((l) => l.content).join('\n')}
        </pre>
      </section>
    </aside>
  )
}
