import { DndContext, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core'
import { useMutation } from 'urql'
import { graphql } from '../gql'
import type { BoardQuery, TaskStatus } from '../gql/graphql'
import { Column } from './Column'
import { TaskCard } from './TaskCard'

const MoveTaskMutation = graphql(`
  mutation MoveTask($id: UUID!, $status: TaskStatus!) {
    moveTask(id: $id, status: $status) {
      id
      status
    }
  }
`)

// Each column says what dropping a card there does
const COLUMNS: { status: TaskStatus; label: string; hint: string }[] = [
  { status: 'TODO', label: 'To Do', hint: 'New tasks wait here' },
  { status: 'IN_PROGRESS', label: 'In Progress', hint: 'Drop a task here to start its agent' },
  { status: 'IN_REVIEW', label: 'In Review', hint: 'Finished work waits for your review' },
  { status: 'DONE', label: 'Done', hint: 'Drop reviewed work here to merge it' },
  { status: 'FAILED', label: 'Failed', hint: 'Runs that went wrong land here' },
]

type Props = {
  tasks: BoardQuery['projects'][number]['tasks']
  selectedId: string | null
  onSelect: (id: string) => void
  onError: (message: string | null) => void
}

export function Board({ tasks, selectedId, onSelect, onError }: Props) {
  const [, moveTask] = useMutation(MoveTaskMutation)

  // Only start a drag after the pointer moves 5px, so a plain click isn't a drag
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }))

  async function handleDragEnd({ active, over }: DragEndEvent) {
    if (!over) return
    const task = tasks.find((t) => t.id === active.id)
    const status = over.id as TaskStatus
    if (!task || task.status === status) return

    const res = await moveTask({ id: task.id, status })
    // Show the backend's rule error, e.g. "cannot move task from Todo to Done"
    onError(res.error ? (res.error.graphQLErrors[0]?.message ?? res.error.message) : null)
  }

  return (
    <DndContext sensors={sensors} onDragEnd={handleDragEnd}>
      <div className="board">
        {COLUMNS.map((col) => {
          const inColumn = tasks.filter((t) => t.status === col.status)
          return (
            <Column key={col.status} {...col} count={inColumn.length}>
              {inColumn.map((t) => (
                <TaskCard
                  key={t.id}
                  id={t.id}
                  title={t.title}
                  description={t.prompt}
                  status={t.status}
                  agent={t.agent}
                  updatedAt={t.updatedAt}
                  headline={t.headline}
                  claim={t.claim}
                  selected={t.id === selectedId}
                  onSelect={onSelect}
                />
              ))}
            </Column>
          )
        })}
      </div>
    </DndContext>
  )
}
