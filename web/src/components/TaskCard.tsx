import { useDraggable } from '@dnd-kit/core'
import type { TaskStatus } from '../gql/graphql'

type Props = {
  id: string
  title: string
  prompt: string
  status: TaskStatus
  agent: string
  updatedAt: string
  selected: boolean
  onSelect: (id: string) => void
}

// "just now", "5m ago", "3h ago", "2d ago"
function timeAgo(iso: string) {
  const minutes = Math.floor((Date.now() - new Date(iso).getTime()) / 60_000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m ago`
  if (minutes < 60 * 24) return `${Math.floor(minutes / 60)}h ago`
  return `${Math.floor(minutes / (60 * 24))}d ago`
}

export function TaskCard({ id, title, prompt, status, agent, updatedAt, selected, onSelect }: Props) {
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
      {prompt && <p className="card-prompt">{prompt}</p>}
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
