import { useMemo, useState } from 'react'
import { DndContext, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core'
import { useMutation, useQuery } from 'urql'
import { Column } from './components/Column'
import { NewTaskForm } from './components/NewTaskForm'
import { TaskCard } from './components/TaskCard'
import { graphql } from './gql'
import type { TaskStatus } from './gql/graphql'

const BoardQuery = graphql(`
  query Board {
    projects {
      id
      name
      tasks {
        id
        title
        status
        agent
      }
    }
  }
`)

const MoveTaskMutation = graphql(`
  mutation MoveTask($id: UUID!, $status: TaskStatus!) {
    moveTask(id: $id, status: $status) {
      id
      status
    }
  }
`)

const COLUMNS: { status: TaskStatus; label: string }[] = [
  { status: 'TODO', label: 'To Do' },
  { status: 'IN_PROGRESS', label: 'In Progress' },
  { status: 'IN_REVIEW', label: 'In Review' },
  { status: 'DONE', label: 'Done' },
  { status: 'FAILED', label: 'Failed' },
]

export default function App() {
  // additionalTypenames: refetch this query whenever a mutation returns a Task
  const context = useMemo(() => ({ additionalTypenames: ['Task'] }), [])
  const [{ data, fetching, error }] = useQuery({ query: BoardQuery, context })
  const [, moveTask] = useMutation(MoveTaskMutation)
  const [moveError, setMoveError] = useState<string | null>(null)

  // Only start a drag after the pointer moves 5px, so a plain click isn't a drag
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }))

  if (fetching && !data) return <p>Loading…</p>
  if (error) return <p>Error: {error.message}</p>

  const project = data?.projects[0]
  if (!project) return <p>No project yet. Create one in GraphiQL first.</p>

  async function handleDragEnd({ active, over }: DragEndEvent) {
    if (!over || !project) return
    const task = project.tasks.find((t) => t.id === active.id)
    const status = over.id as TaskStatus
    if (!task || task.status === status) return

    const res = await moveTask({ id: task.id, status })
    // Show the backend's rule error, e.g. "cannot move task from Todo to Done"
    setMoveError(res.error ? (res.error.graphQLErrors[0]?.message ?? res.error.message) : null)
  }

  return (
    <main>
      <h1>{project.name}</h1>
      <NewTaskForm projectId={project.id} />
      {moveError && <p className="error">{moveError}</p>}

      <DndContext sensors={sensors} onDragEnd={handleDragEnd}>
        <div className="board">
          {COLUMNS.map((col) => (
            <Column key={col.status} status={col.status} label={col.label}>
              {project.tasks
                .filter((t) => t.status === col.status)
                .map((t) => (
                  <TaskCard key={t.id} id={t.id} title={t.title} agent={t.agent} />
                ))}
            </Column>
          ))}
        </div>
      </DndContext>
    </main>
  )
}
