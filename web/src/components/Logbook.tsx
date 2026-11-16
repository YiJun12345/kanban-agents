import { useState } from 'react'
import { useQuery } from 'urql'
import { graphql } from '../gql'
import { clock, dayLabel, FILTERS, KIND_ICON, ticket, type Filter, WHO } from '../journal'
import { useRefetchOnTick } from '../useRefetchOnTick'
import { Icon } from './Icon'
import { FilterChips } from './Journal'

const LogbookQuery = graphql(`
  query Logbook($projectId: UUID!) {
    project(id: $projectId) {
      id
      journal(limit: 200) {
        id
        seq
        runId
        actor
        kind
        message
        details
        createdAt
        task {
          id
          title
        }
      }
    }
  }
`)

type Props = {
  projectId: string
  tick: number // goes up with every live event; see useRefetchOnTick
  selectedId: string | null
  onSelect: (taskId: string) => void
}

/** Every task's journal in one list, newest first: what happened in this project, and when */
export function Logbook({ projectId, tick, selectedId, onSelect }: Props) {
  const [{ data, fetching, stale }, refetch] = useQuery({
    query: LogbookQuery,
    variables: { projectId },
    requestPolicy: 'cache-and-network',
  })
  useRefetchOnTick(tick, fetching || stale, refetch)
  const [filter, setFilter] = useState<Filter>('All')

  const entries = (data?.project?.journal ?? []).filter(FILTERS[filter])

  // Entries arrive newest first; put each under its day's heading
  const days: { label: string; entries: typeof entries }[] = []
  for (const entry of entries) {
    const label = dayLabel(entry.createdAt)
    if (days.at(-1)?.label !== label) days.push({ label, entries: [] })
    days.at(-1)!.entries.push(entry)
  }

  return (
    <div className="logbook">
      <div className="logbook-head">
        <p className="lede">Everything people and agents did in this project, newest first.</p>
        <FilterChips value={filter} onChange={setFilter} />
      </div>

      {entries.length === 0 && (
        <p className="journal-empty">{fetching ? 'Loading…' : 'Nothing here yet.'}</p>
      )}

      {days.map((day) => (
        <section key={day.label} className="logbook-day">
          <h3>{day.label}</h3>
          <ol>
            {day.entries.map((e) => (
              <li
                key={e.id}
                className={e.task.id === selectedId ? 'log-row selected' : 'log-row'}
                data-kind={e.kind}
                data-actor={e.actor}
              >
                <button onClick={() => onSelect(e.task.id)}>
                  <time dateTime={e.createdAt}>{clock(e.createdAt)}</time>
                  <span className="entry-icon">
                    <Icon name={KIND_ICON[e.kind]} size={13} />
                  </span>
                  <span className="log-who">{WHO[e.actor]}</span>
                  <span className="log-message">{e.message}</span>
                  <span className="log-task">
                    <span className="ticket">{ticket(e.task.id)}</span> {e.task.title}
                  </span>
                </button>
              </li>
            ))}
          </ol>
        </section>
      ))}
    </div>
  )
}
