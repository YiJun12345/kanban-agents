import { useDroppable } from '@dnd-kit/core'
import type { ReactNode } from 'react'
import type { TaskStatus } from '../gql/graphql'

type Props = {
  status: TaskStatus
  label: string
  children: ReactNode
}

export function Column({ status, label, children }: Props) {
  // The column's status is its drop id, so onDragEnd knows where the card landed
  const { setNodeRef, isOver } = useDroppable({ id: status })

  return (
    <section ref={setNodeRef} className={isOver ? 'column over' : 'column'}>
      <h2>{label}</h2>
      {children}
    </section>
  )
}
