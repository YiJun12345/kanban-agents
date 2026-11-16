import { useQuery, useSubscription } from 'urql'
import { graphql } from '../gql'
import { ticket } from '../journal'
import { timeAgo } from '../time'
import { Icon } from './Icon'
import { useJournal } from '../useJournal'
import { useRefetchOnTick } from '../useRefetchOnTick'
import { Activity } from './Journal'
import { ReviewActions } from './ReviewActions'

const TaskQuery = graphql(`
  query TaskPanel($id: UUID!) {
    task(id: $id) {
      id
      title
      status
      prompt
      agent
      branch
      mergeCommit
      updatedAt
      diff
      logs {
        runId
        seq
        content
      }
      journal {
        id
        seq
        runId
        actor
        kind
        message
        details
        createdAt
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

type Props = {
  taskId: string
  tick: number // goes up with every live event; see useRefetchOnTick
  baseBranch: string
  onClose: () => void
}

export function TaskPanel({ taskId, tick, baseBranch, onClose }: Props) {
  // Saved data from the database, refetched after every live event
  const [{ data, fetching, stale }, refetch] = useQuery({
    query: TaskQuery,
    variables: { id: taskId },
    requestPolicy: 'cache-and-network',
  })
  useRefetchOnTick(tick, fetching || stale, refetch)

  // New lines as the agent prints them; the reducer appends each one to a list
  const [live] = useSubscription(
    { query: TaskLogsSubscription, variables: { taskId } },
    (lines: Line[] = [], event) => [...lines, event.taskLogs],
  )

  // Every hook must run on every render, so this comes before the early return below
  const entries = useJournal(taskId, data?.task?.journal ?? [])

  const task = data?.task
  if (!task) return null

  // Saved + live lines can overlap, so merge them by seq and only show the newest run
  const all = [...task.logs, ...(live.data ?? [])]
  const latestRun = all.at(-1)?.runId
  const bySeq = new Map(all.filter((l) => l.runId === latestRun).map((l) => [l.seq, l]))
  const lines = [...bySeq.values()].sort((a, b) => a.seq - b.seq)

  // The agent's report for its latest run: the first thing a reviewer reads
  const summary = entries.findLast((e) => e.kind === 'SUMMARY')
  const filesChanged = task.diff?.match(/^diff --git/gm)?.length ?? 0

  return (
    <aside className="panel" aria-label={`Task ${task.title}`}>
      <header>
        <div className="panel-top">
          <span className="ticket">{ticket(task.id)}</span>
          <span className="stamp" data-status={task.status}>
            {STATUS_LABEL[task.status]}
          </span>
          <button className="icon-button" onClick={onClose} aria-label="Close">
            <Icon name="close" size={16} />
          </button>
        </div>
        <h2>{task.title}</h2>
      </header>

      <ReviewActions
        taskId={task.id}
        status={task.status}
        mergeCommit={task.mergeCommit}
        baseBranch={baseBranch}
      />

      <dl className="details">
        <dt>Assignee</dt>
        <dd className="assignee">
          <span className="avatar">AI</span> {task.agent.toLowerCase()} agent
        </dd>
        <dt>Branch</dt>
        <dd>
          {!task.branch ? (
            'not started'
          ) : task.mergeCommit ? (
            <>
              <code>{task.branch}</code> <span className="muted">(merged, deleted)</span>
            </>
          ) : (
            <code>{task.branch}</code>
          )}
        </dd>
        <dt>Updated</dt>
        <dd>{timeAgo(task.updatedAt)}</dd>
      </dl>

      <section>
        <h3>Description</h3>
        <p className="description">{task.prompt || 'No description.'}</p>
      </section>

      {summary && (
        <section>
          <h3>
            Agent summary
            <span className="aside">{timeAgo(summary.createdAt)}</span>
          </h3>
          <p className="summary">{summary.message}</p>
        </section>
      )}

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

      <Activity
        taskId={task.id}
        entries={entries}
        running={task.status === 'IN_PROGRESS'}
        rawOutput={lines.map((l) => l.content).join('\n')}
      />
    </aside>
  )
}
