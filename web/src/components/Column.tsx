import { useDroppable } from '@dnd-kit/core'
import type { ReactNode } from 'react'
import type { TaskStatus } from '../gql/graphql'

type Props = {
  status: TaskStatus
  label: string
  count: number
  children: ReactNode
}

export function Column({ status, label, count, children }: Props) {
  // The column's status is its drop id, so onDragEnd knows where the card landed
  const { setNodeRef, isOver } = useDroppable({ id: status })

  return (
    // data-status lets the CSS pick this column's colour
    <section ref={setNodeRef} className={isOver ? 'column over' : 'column'} data-status={status}>
      <header>
        <span className="dot" />
        <h2>{label}</h2>
        <span className="count">{count}</span>
      </header>
      <div className="cards">{count === 0 ? <p className="empty">Drop tasks here</p> : children}</div>
    </section>
  )
}
