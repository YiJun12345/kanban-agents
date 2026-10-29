import { useDraggable } from '@dnd-kit/core'

type Props = {
  id: string
  title: string
  agent: string
  selected: boolean
  onSelect: (id: string) => void
}

export function TaskCard({ id, title, agent, selected, onSelect }: Props) {
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
      onClick={() => onSelect(id)}
      {...listeners}
      {...attributes}
    >
      <strong>{title}</strong>
      <small>{agent}</small>
    </article>
  )
}
