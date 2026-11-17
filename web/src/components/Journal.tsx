import { useState, type FormEvent, type ReactNode } from 'react'
import { useMutation } from 'urql'
import { graphql } from '../gql'
import { clock, FILTERS, KIND_ICON, who, type Entry, type Filter } from '../journal'
import { Icon } from './Icon'

const AddNoteMutation = graphql(`
  mutation AddJournalNote($taskId: UUID!, $message: String!) {
    addJournalNote(taskId: $taskId, message: $message) {
      id
    }
  }
`)

// --- grouping ---

// The timeline is a list of items: an entry on its own (e.g. you moved the card),
// or a run, which holds everything that happened during one agent run
type Run = { type: 'run'; id: string; number: number; entries: Entry[] }
type Item = { type: 'entry'; entry: Entry } | Run

function groupByRun(entries: Entry[]): Item[] {
  const items: Item[] = []
  const runs = new Map<string, Run>()
  for (const entry of entries) {
    if (!entry.runId) {
      items.push({ type: 'entry', entry })
      continue
    }
    let run = runs.get(entry.runId)
    if (!run) {
      // A run appears in the timeline where its first entry is
      run = { type: 'run', id: entry.runId, number: runs.size + 1, entries: [] }
      runs.set(entry.runId, run)
      items.push(run)
    }
    run.entries.push(entry)
  }
  return items
}

function applyFilter(items: Item[], filter: Filter): Item[] {
  const keep = FILTERS[filter]
  return items.flatMap((item): Item[] => {
    if (item.type === 'entry') return keep(item.entry) ? [item] : []
    const entries = item.entries.filter(keep)
    return entries.length ? [{ ...item, entries }] : [] // hide runs with nothing left
  })
}

// --- components ---

export function FilterChips({ value, onChange }: { value: Filter; onChange: (f: Filter) => void }) {
  return (
    <div className="filters" role="group" aria-label="Filter activity">
      {(Object.keys(FILTERS) as Filter[]).map((name) => (
        <button key={name} aria-pressed={value === name} onClick={() => onChange(name)}>
          {name}
        </button>
      ))}
    </div>
  )
}

type ActivityProps = {
  taskId: string
  entries: Entry[]
  running: boolean
  rawOutput: string // the latest run's raw output
}

export function Activity({ taskId, entries, running, rawOutput }: ActivityProps) {
  const [filter, setFilter] = useState<Filter>('All')
  const all = groupByRun(entries)
  const latestRun = all.findLast((item) => item.type === 'run')?.id
  const items = applyFilter(all, filter)

  return (
    <section>
      <h3>
        Activity
        {running && (
          <span className="live">
            <span className="pulse" /> live
          </span>
        )}
      </h3>
      <FilterChips value={filter} onChange={setFilter} />

      {items.length === 0 ? (
        <p className="journal-empty">
          {entries.length === 0
            ? 'Nothing yet. Activity appears as the task moves and the agent works.'
            : `No ${filter.toLowerCase()} yet.`}
        </p>
      ) : (
        <ol className="journal">
          {items.map((item) =>
            item.type === 'entry' ? (
              <EntryItem key={item.entry.id} entry={item.entry} />
            ) : (
              <RunItem key={item.id} run={item} latest={item.id === latestRun}>
                {item.id === latestRun && <RawOutput text={rawOutput} running={running} />}
              </RunItem>
            ),
          )}
        </ol>
      )}

      <NoteForm taskId={taskId} />
    </section>
  )
}

export function EntryItem({ entry: e }: { entry: Entry }) {
  const feedback = e.details.review === 'changes_requested'
  return (
    <li className="entry" data-kind={e.kind} data-actor={e.actor} data-feedback={feedback || undefined}>
      <span className="entry-icon">
        <Icon name={feedback ? 'redo' : KIND_ICON[e.kind]} size={13} />
      </span>
      <div className="entry-body">
        <div className="entry-meta">
          <strong>{who(e)}</strong>
          {feedback && <span className="tag">requested changes</span>}
          {e.kind === 'SUMMARY' && <span className="tag">summary</span>}
          {e.kind === 'CHECKPOINT' && <span className="tag">checkpoint</span>}
          {e.noteKind && (
            <span className="tag" data-note={e.noteKind}>
              {e.noteKind.toLowerCase()}
            </span>
          )}
          <time dateTime={e.createdAt}>{clock(e.createdAt)}</time>
        </div>
        <p>{e.message}</p>
        {e.kind === 'CHECKPOINT' && (
          // A checkpoint also says what the worker is trying to do and what comes next
          <dl className="checkpoint">
            {typeof e.details.intent === 'string' && e.details.intent && (
              <>
                <dt>Intent</dt>
                <dd>{e.details.intent}</dd>
              </>
            )}
            {typeof e.details.next_action === 'string' && e.details.next_action && (
              <>
                <dt>Next</dt>
                <dd>{e.details.next_action}</dd>
              </>
            )}
          </dl>
        )}
        {Array.isArray(e.details.files) && (
          <ul className="files">
            {e.details.files.map((f) => (
              <li key={String(f)}>
                <code>{String(f)}</code>
              </li>
            ))}
          </ul>
        )}
      </div>
    </li>
  )
}

/** One agent run, folded into a single row; the latest run starts open */
function RunItem({ run, latest, children }: { run: Run; latest: boolean; children: ReactNode }) {
  const finished = run.entries.find((e) => e.kind === 'RUN_FINISHED')
  const commit = run.entries.find((e) => e.kind === 'COMMIT')?.details.commit
  const secs = finished?.details.duration_secs
  // Which worker did this run (the built-in runners are not worth naming)
  const worker = run.entries.find((e) => e.kind === 'RUN_STARTED')?.details.worker
  const outside = typeof worker === 'string' && !worker.startsWith('builtin-')
  let state = 'running'
  if (finished) {
    if (finished.details.stopped) state = 'stopped'
    else state = finished.details.exit_code === 0 ? 'finished' : 'failed'
  }

  return (
    <li className="run" data-state={state}>
      <details open={latest}>
        <summary>
          <Icon name="chevron" size={12} />
          <span className="run-title">Run #{run.number}</span>
          {outside && <span className="run-worker">{worker}</span>}
          <span className="run-state">{state}</span>
          {typeof secs === 'number' && <span className="run-time">{secs.toFixed(1)}s</span>}
          {typeof commit === 'string' && (
            <code className="run-commit">
              <Icon name="commit" size={12} /> {commit}
            </code>
          )}
        </summary>
        <ol className="journal">
          {run.entries.map((e) => (
            <EntryItem key={e.id} entry={e} />
          ))}
        </ol>
        {children}
      </details>
    </li>
  )
}

/** The agent's raw output: hidden behind a click unless the agent is running right now */
function RawOutput({ text, running }: { text: string; running: boolean }) {
  return (
    <details className="raw-output" open={running}>
      <summary>View raw output</summary>
      <pre className="terminal">{text || 'No output yet.'}</pre>
    </details>
  )
}

function NoteForm({ taskId }: { taskId: string }) {
  const [message, setMessage] = useState('')
  const [result, addNote] = useMutation(AddNoteMutation)

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    if (!message.trim()) return
    const res = await addNote({ taskId, message })
    if (!res.error) setMessage('')
  }

  return (
    <form className="note-form" onSubmit={handleSubmit}>
      <input aria-label="Note" placeholder="Add a note…" value={message} onChange={(e) => setMessage(e.target.value)} />
      <button disabled={result.fetching || !message.trim()}>Add note</button>
    </form>
  )
}
