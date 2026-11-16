import { useDraggable } from '@dnd-kit/core'
import type { JournalActor, JournalKind, TaskStatus } from '../gql/graphql'
import { KIND_ICON, ticket, WHO } from '../journal'
import { timeAgo } from '../time'
import { Icon } from './Icon'

type Props = {
  id: string
  title: string
  description: string
  status: TaskStatus
  agent: string
  updatedAt: string
  // The newest note, summary or error from the task's journal
  headline?: { actor: JournalActor; kind: JournalKind; message: string } | null
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
          <span className="working">
            <span className="spinner" /> working
          </span>
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
            <b>{WHO[headline.actor]}:</b> {headline.message}
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
