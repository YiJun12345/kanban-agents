import { useState, type FormEvent } from 'react'
import { useMutation } from 'urql'
import { graphql } from '../gql'
import type { TaskStatus } from '../gql/graphql'
import { Icon } from './Icon'

const MoveMutation = graphql(`
  mutation PanelMoveTask($id: UUID!, $status: TaskStatus!) {
    moveTask(id: $id, status: $status) {
      id
      status
    }
  }
`)

const ApproveMutation = graphql(`
  mutation ApproveTask($id: UUID!) {
    approveTask(id: $id) {
      id
      status
      mergeCommit
    }
  }
`)

const RequestChangesMutation = graphql(`
  mutation RequestChanges($id: UUID!, $feedback: String!) {
    requestChanges(id: $id, feedback: $feedback) {
      id
      status
    }
  }
`)

type Props = {
  taskId: string
  status: TaskStatus
  claimed: boolean // is a worker on it right now?
  mergeCommit?: string | null
  baseBranch: string
}

/** The buttons for the task's next step. They do the same as dragging the card. */
export function ReviewActions({ taskId, status, claimed, mergeCommit, baseBranch }: Props) {
  const [moving, move] = useMutation(MoveMutation)
  const [approving, approve] = useMutation(ApproveMutation)
  const [requesting, requestChanges] = useMutation(RequestChangesMutation)
  const [writing, setWriting] = useState(false) // is the feedback box open?
  const [feedback, setFeedback] = useState('')
  const [error, setError] = useState<string | null>(null)

  const busy = moving.fetching || approving.fetching || requesting.fetching

  // Run a mutation and show the server's own message if it refuses
  async function run(action: Promise<{ error?: { graphQLErrors: { message: string }[]; message: string } }>) {
    const res = await action
    setError(res.error ? (res.error.graphQLErrors[0]?.message ?? res.error.message) : null)
    return !res.error
  }

  async function sendFeedback(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    if (await run(requestChanges({ id: taskId, feedback }))) {
      setFeedback('')
      setWriting(false)
    }
  }

  return (
    <div className="actions" data-status={status}>
      {status === 'TODO' && (
        <button className="primary" disabled={busy} onClick={() => run(move({ id: taskId, status: 'IN_PROGRESS' }))}>
          <Icon name="play" /> Start agent
        </button>
      )}

      {status === 'IN_PROGRESS' && (
        <button disabled={busy} onClick={() => run(move({ id: taskId, status: 'TODO' }))}>
          <Icon name="stop" /> {claimed ? 'Stop agent' : 'Back to To Do'}
        </button>
      )}

      {status === 'IN_REVIEW' && !writing && (
        <>
          <button className="primary" disabled={busy} onClick={() => run(approve({ id: taskId }))}>
            <Icon name="check" /> {approving.fetching ? 'Merging…' : 'Approve & merge'}
          </button>
          <button disabled={busy} onClick={() => setWriting(true)}>
            <Icon name="redo" /> Request changes
          </button>
        </>
      )}

      {status === 'IN_REVIEW' && writing && (
        <form className="feedback-form" onSubmit={sendFeedback}>
          <textarea
            autoFocus
            rows={3}
            placeholder="What should the agent change?"
            value={feedback}
            onChange={(e) => setFeedback(e.target.value)}
          />
          <div>
            <button className="primary" disabled={busy || !feedback.trim()}>
              Send back to agent
            </button>
            <button type="button" onClick={() => setWriting(false)}>
              Cancel
            </button>
          </div>
        </form>
      )}

      {status === 'FAILED' && (
        <>
          <button className="primary" disabled={busy} onClick={() => run(move({ id: taskId, status: 'IN_PROGRESS' }))}>
            <Icon name="redo" /> Retry
          </button>
          <button disabled={busy} onClick={() => run(move({ id: taskId, status: 'TODO' }))}>
            Back to To Do
          </button>
        </>
      )}

      {status === 'DONE' && (
        <p className="merged">
          <Icon name="merge" />
          {mergeCommit ? (
            <>
              Merged into <code>{baseBranch}</code> as <code>{mergeCommit}</code>
            </>
          ) : (
            'Approved. There was nothing to merge.'
          )}
        </p>
      )}

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </div>
  )
}
