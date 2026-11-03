import { useMemo, useState } from 'react'
import { DndContext, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core'
import { useMutation, useQuery, useSubscription } from 'urql'
import { Column } from './components/Column'
import { LogPanel } from './components/LogPanel'
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
        prompt
        status
        agent
        updatedAt
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

const TaskUpdatedSubscription = graphql(`
  subscription TaskUpdated($projectId: UUID!) {
    taskUpdated(projectId: $projectId) {
      id
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
  const [selectedId, setSelectedId] = useState<string | null>(null)

  // Live updates: whenever a task changes on the server, refetch the board.
  // additionalTypenames tells urql's cache "this event affects Task data".
  const projectId = data?.projects[0]?.id
  useSubscription({
    query: TaskUpdatedSubscription,
    variables: { projectId: projectId ?? '' },
    pause: !projectId, // wait until we know which project to watch
    context,
  })


  // Only start a drag after the pointer moves 5px, so a plain click isn't a drag
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }))

  if (fetching && !data) return <p className="message">Loading…</p>
  if (error) return <p className="message error">Error: {error.message}</p>

  const project = data?.projects[0]
  if (!project) return <p className="message">No project yet. Create one in GraphiQL first.</p>

  async function handleDragEnd({ active, over }: DragEndEvent) {
    if (!over || !project) return
    const task = project.tasks.find((t) => t.id === active.id)
    const status = over.id as TaskStatus
    if (!task || task.status === status) return

    const res = await moveTask({ id: task.id, status })
    // Show the backend's rule error, e.g. "cannot move task from Todo to Done"
    setMoveError(res.error ? (res.error.graphQLErrors[0]?.message ?? res.error.message) : null)
  }

  const running = project.tasks.filter((t) => t.status === 'IN_PROGRESS').length

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="logo" aria-hidden>
            ▦
          </span>
          Kanban Agents
        </div>
        <div className="stats">
          <span>{project.tasks.length} tasks</span>
          {running > 0 && (
            <span className="running">
              <span className="pulse" /> {running} agent{running > 1 ? 's' : ''} running
            </span>
          )}
        </div>
      </header>

      <main>
        <div className="board-header">
          <h1>{project.name}</h1>
          <NewTaskForm projectId={project.id} />
        </div>

        <div className="layout">
          <DndContext sensors={sensors} onDragEnd={handleDragEnd}>
            <div className="board">
              {COLUMNS.map((col) => {
                const tasks = project.tasks.filter((t) => t.status === col.status)
                return (
                  <Column key={col.status} status={col.status} label={col.label} count={tasks.length}>
                    {tasks.map((t) => (
                      <TaskCard
                        key={t.id}
                        id={t.id}
                        title={t.title}
                        prompt={t.prompt}
                        status={t.status}
                        agent={t.agent}
                        updatedAt={t.updatedAt}
                        selected={t.id === selectedId}
                        onSelect={setSelectedId}
                      />
                    ))}
                  </Column>
                )
              })}
            </div>
          </DndContext>

          {selectedId && <LogPanel taskId={selectedId} onClose={() => setSelectedId(null)} />}
        </div>
      </main>

      {moveError && (
        <div className="toast" role="alert">
          <p className="error">{moveError}</p>
          <button onClick={() => setMoveError(null)} aria-label="Dismiss">
            ✕
          </button>
        </div>
      )}
    </div>
  )
}
