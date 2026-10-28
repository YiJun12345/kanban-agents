import { useDraggable } from '@dnd-kit/core'

type Props = {
  id: string
  title: string
  agent: string
}

export function TaskCard({ id, title, agent }: Props) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({ id })

  // While dragging, dnd-kit tells us how far the pointer moved; we shift the card by that much
  const style = transform
    ? { transform: `translate3d(${transform.x}px, ${transform.y}px, 0)` }
    : undefined

  return (
    <article
      ref={setNodeRef}
      style={style}
      className={isDragging ? 'card dragging' : 'card'}
      {...listeners}
      {...attributes}
    >
      <strong>{title}</strong>
      <small>{agent}</small>
    </article>
  )
}
