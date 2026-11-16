import { useMemo, useState } from 'react'
import { useQuery, useSubscription } from 'urql'
import { Board } from './components/Board'
import { Icon } from './components/Icon'
import { Logbook } from './components/Logbook'
import { NewTaskForm } from './components/NewTaskForm'
import { NewProjectForm, ProjectSwitcher } from './components/ProjectSwitcher'
import { TaskPanel } from './components/TaskPanel'
import { graphql } from './gql'
import { useRefetchOnTick } from './useRefetchOnTick'

const BoardQuery = graphql(`
  query Board {
    projects {
      id
      name
      repoPath
      defaultBranch
      tasks {
        id
        title
        prompt
        status
        agent
        updatedAt
        headline {
          actor
          kind
          message
        }
      }
    }
    agents {
      kind
      label
      ready
      reason
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

type View = 'board' | 'logbook'

// Remember the chosen project between visits (localStorage can be unavailable, e.g. private mode)
const PROJECT_KEY = 'kanban.project'
function savedProject() {
  try {
    return localStorage.getItem(PROJECT_KEY)
  } catch {
    return null
  }
}

export default function App() {
  // additionalTypenames: refetch this query whenever a mutation returns a Task or a Project
  const context = useMemo(() => ({ additionalTypenames: ['Task', 'Project'] }), [])
  const [{ data, fetching, stale, error }, refetch] = useQuery({ query: BoardQuery, context })
  const [projectId, setProjectId] = useState<string | null>(savedProject)
  const [view, setView] = useState<View>('board')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [moveError, setMoveError] = useState<string | null>(null)

  // The chosen project; otherwise the first one that has tasks, otherwise the first one
  const projects = data?.projects ?? []
  const project =
    projects.find((p) => p.id === projectId) ?? projects.find((p) => p.tasks.length > 0) ?? projects[0]

  // Live updates: the server tells us whenever a task in this project changes.
  // We only count the events; `tick` going up makes every query on the page refetch.
  const [live] = useSubscription(
    {
      query: TaskUpdatedSubscription,
      variables: { projectId: project?.id ?? '' },
      pause: !project, // wait until we know which project to watch
    },
    (count: number = 0) => count + 1,
  )
  const tick = live.data ?? 0
  useRefetchOnTick(tick, fetching || stale, refetch)

  function chooseProject(id: string) {
    setProjectId(id)
    setSelectedId(null)
    try {
      localStorage.setItem(PROJECT_KEY, id)
    } catch {
      // not being able to remember the choice is fine
    }
  }

  if (fetching && !data) return <p className="message">Loading…</p>
  if (error) return <p className="message error">Error: {error.message}</p>

  if (!project) {
    return (
      <div className="welcome">
        <h1>Kanban Agents</h1>
        <p className="lede">
          A board where the team members are AI coding agents. Start by pointing it at a git repository.
        </p>
        <NewProjectForm onCreated={chooseProject} />
      </div>
    )
  }

  const running = project.tasks.filter((t) => t.status === 'IN_PROGRESS').length
  const waiting = project.tasks.filter((t) => t.status === 'IN_REVIEW').length

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="logo" aria-hidden>
            <i />
            <i />
            <i />
          </span>
          Kanban Agents
        </div>

        <ProjectSwitcher projects={projects} selectedId={project.id} onSelect={chooseProject} />

        <nav className="tabs" aria-label="View">
          <button aria-current={view === 'board' ? 'page' : undefined} onClick={() => setView('board')}>
            Board
          </button>
          <button aria-current={view === 'logbook' ? 'page' : undefined} onClick={() => setView('logbook')}>
            Logbook
          </button>
        </nav>

        <div className="stats">
          <span>{project.tasks.length} tasks</span>
          {waiting > 0 && <span className="waiting">{waiting} to review</span>}
          {running > 0 && (
            <span className="running">
              <span className="pulse" /> {running} agent{running > 1 ? 's' : ''} running
            </span>
          )}
        </div>
      </header>

      <main>
        <div className="board-header">
          <div>
            <h1>{project.name}</h1>
            <p className="repo">
              <code>{project.repoPath}</code>
              <span className="branch">
                <Icon name="merge" size={12} /> {project.defaultBranch}
              </span>
            </p>
          </div>
          {view === 'board' && <NewTaskForm projectId={project.id} agents={data?.agents ?? []} />}
        </div>

        <div className={selectedId ? 'layout with-panel' : 'layout'}>
          {view === 'board' ? (
            <Board tasks={project.tasks} selectedId={selectedId} onSelect={setSelectedId} onError={setMoveError} />
          ) : (
            <Logbook projectId={project.id} tick={tick} selectedId={selectedId} onSelect={setSelectedId} />
          )}

          {selectedId && (
            <TaskPanel
              key={selectedId} // a fresh panel per task, so one task's live lines never show in another
              taskId={selectedId}
              tick={tick}
              baseBranch={project.defaultBranch}
              onClose={() => setSelectedId(null)}
            />
          )}
        </div>
      </main>

      {moveError && (
        <div className="toast" role="alert">
          <p className="error">{moveError}</p>
          <button className="icon-button" onClick={() => setMoveError(null)} aria-label="Dismiss">
            <Icon name="close" />
          </button>
        </div>
      )}
    </div>
  )
}
