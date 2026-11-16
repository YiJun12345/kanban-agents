import { useMemo, useState, type FormEvent } from 'react'
import { useMutation, useQuery } from 'urql'
import { graphql } from '../gql'
import { ticket } from '../journal'
import { timeAgo } from '../time'
import { useRefetchOnTick } from '../useRefetchOnTick'

const WorkersQuery = graphql(`
  query Workers {
    workers {
      id
      name
      builtin
      lastSeenAt
      claims {
        quietSecs
        task {
          id
          title
        }
      }
    }
  }
`)

const RegisterWorkerMutation = graphql(`
  mutation RegisterWorker($name: String!) {
    registerWorker(name: $name) {
      token
      worker {
        id
        name
      }
    }
  }
`)

type Props = {
  tick: number // goes up with every live event; see useRefetchOnTick
  onSelect: (taskId: string) => void
}

/** Everything that takes work from the board, and how to add an agent of your own */
export function Workers({ tick, onSelect }: Props) {
  // 'Worker': refetch when a mutation returns a Worker, even while the list is empty
  const context = useMemo(() => ({ additionalTypenames: ['Worker'] }), [])
  const [{ data, fetching, stale }, refetch] = useQuery({
    query: WorkersQuery,
    requestPolicy: 'cache-and-network',
    context,
  })
  useRefetchOnTick(tick, fetching || stale, refetch)

  const workers = data?.workers ?? []

  return (
    <div className="workers">
      <p className="lede">
        A worker is anything that takes tasks from the board: the built-in runners, or an agent of your own
        that uses the <code>kanban</code> command.
      </p>

      <table>
        <thead>
          <tr>
            <th>Worker</th>
            <th>Working on</th>
            <th>Last seen</th>
          </tr>
        </thead>
        <tbody>
          {workers.map((w) => (
            <tr key={w.id}>
              <td>
                <code>{w.name}</code> {w.builtin && <span className="tag">built in</span>}
              </td>
              <td>
                {w.claims.length === 0 && <span className="muted">nothing</span>}
                {w.claims.map((c) => (
                  <button key={c.task.id} className="link" onClick={() => onSelect(c.task.id)}>
                    <span className="ticket">{ticket(c.task.id)}</span> {c.task.title}
                    <span className="muted"> · heartbeat {c.quietSecs}s ago</span>
                  </button>
                ))}
              </td>
              <td className="muted">{w.lastSeenAt ? timeAgo(w.lastSeenAt) : 'never'}</td>
            </tr>
          ))}
          {workers.length === 0 && (
            <tr>
              <td colSpan={3} className="muted">
                {fetching ? 'Loading…' : 'No workers yet.'}
              </td>
            </tr>
          )}
        </tbody>
      </table>

      <RegisterWorker onCreated={() => refetch({ requestPolicy: 'network-only' })} />
    </div>
  )
}

function RegisterWorker({ onCreated }: { onCreated: () => void }) {
  const [name, setName] = useState('')
  const [result, register] = useMutation(RegisterWorkerMutation)
  // The token only exists in this component's memory; leaving the page forgets it
  const [created, setCreated] = useState<{ name: string; token: string } | null>(null)

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    const res = await register({ name })
    if (res.data) {
      setCreated({ name: res.data.registerWorker.worker.name, token: res.data.registerWorker.token })
      setName('')
      onCreated() // show the new worker in the list above
    }
  }

  return (
    <section className="register-worker">
      <h3>Add a worker</h3>
      <form onSubmit={handleSubmit}>
        <input
          className="mono"
          aria-label="Worker name"
          placeholder="worker-1"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button className="primary" disabled={result.fetching || !name.trim()}>
          Create token
        </button>
        {result.error && (
          <span className="error" role="alert">
            {result.error.graphQLErrors[0]?.message ?? result.error.message}
          </span>
        )}
      </form>

      {created && (
        <div className="token" role="status">
          <p>
            Token for <code>{created.name}</code>. Copy it now: it is not stored and cannot be shown again.
          </p>
          <pre className="terminal">{`export KANBAN_TOKEN=${created.token}

kanban next                          # claim the next waiting task
kanban note <id> --kind plan "..."   # write in its journal
kanban submit <id> --summary "..."   # commit and send to review

# or let one command do all of it:
kanban work -- <your agent command>`}</pre>
        </div>
      )}
    </section>
  )
}
