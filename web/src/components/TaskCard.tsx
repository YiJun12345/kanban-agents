import { useDraggable } from '@dnd-kit/core'
import type { TaskStatus } from '../gql/graphql'
import { timeAgo } from '../time'

type Props = {
  id: string
  title: string
  description: string
  status: TaskStatus
  agent: string
  updatedAt: string
  selected: boolean
  onSelect: (id: string) => void
}

export function TaskCard({ id, title, description, status, agent, updatedAt, selected, onSelect }: Props) {
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
      <strong>{title}</strong>
      {description && <p className="card-description">{description}</p>}
      <footer>
        <span className="badge">{agent.toLowerCase()}</span>
        {status === 'IN_PROGRESS' ? (
          <span className="working">
            <span className="spinner" /> working
          </span>
        ) : (
          <span className="time">{timeAgo(updatedAt)}</span>
        )}
      </footer>
    </article>
  )
}
