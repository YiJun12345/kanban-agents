import { useDraggable } from '@dnd-kit/core'
import type { JournalActor, JournalKind, TaskStatus } from '../gql/graphql'
import { KIND_ICON, ticket, who } from '../journal'
import { timeAgo } from '../time'
import { Icon } from './Icon'

/** A worker that has sent no heartbeat for this long is probably stuck (its claim expires at 60s) */
const QUIET_AFTER_SECS = 30

/** Top-right of a card in In Progress: who has it, or that nobody has yet */
function Holder({ claim }: { claim: Props['claim'] }) {
  if (!claim) return <span className="waiting-worker">waiting for a worker</span>
  const name = claim.worker.builtin ? 'working' : claim.worker.name
  if (claim.quietSecs > QUIET_AFTER_SECS) {
    return (
      <span className="quiet" title="No heartbeat lately; the claim expires after 60s of silence">
        {name} · quiet {claim.quietSecs}s
      </span>
    )
  }
  return (
    <span className="working">
      <span className="spinner" /> {name}
    </span>
  )
}

type Props = {
  id: string
  title: string
  description: string
  status: TaskStatus
  agent: string
  updatedAt: string
  // The newest note, summary or error from the task's journal
  headline?: { actor: JournalActor; kind: JournalKind; author?: string | null; message: string } | null
  // Who is working on it right now, and how long since they last said so
  claim?: { quietSecs: number; worker: { name: string; builtin: boolean } } | null
  selected: boolean
  onSelect: (id: string) => void
}

export function TaskCard({
  id,
  title,
  description,
  status,
  agent,
  updatedAt,
  headline,
  claim,
  selected,
  onSelect,
}: Props) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({ id })

  // While dragging, dnd-kit tells us how far the pointer moved; we shift the card by that much
  const style = transform
    ? { transform: `translate3d(${transform.x}px, ${transform.y}px, 0)` }
    : undefined

  const classes = ['card', isDragging && 'dragging', selected && 'selected'].filter(Boolean)

  return (
    <article
      ref={setNodeRef}
      style={style}
      className={classes.join(' ')}
      data-status={status}
      onClick={() => onSelect(id)}
      {...listeners}
      {...attributes}
    >
      <div className="card-top">
        <span className="ticket">{ticket(id)}</span>
        {status === 'IN_PROGRESS' ? (
          <Holder claim={claim} />
        ) : (
          <span className="time">{timeAgo(updatedAt)}</span>
        )}
      </div>
      <strong className="card-title">{title}</strong>
      {description && <p className="card-description">{description}</p>}
      {headline && (
        <p className="card-headline" data-kind={headline.kind} data-actor={headline.actor} title={headline.message}>
          <Icon name={KIND_ICON[headline.kind]} size={12} />
          <span>
            <b>{who(headline)}:</b> {headline.message}
          </span>
        </p>
      )}
      <footer>
        <span className="agent-chip" data-agent={agent}>
          {agent.toLowerCase()}
        </span>
      </footer>
    </article>
  )
}
