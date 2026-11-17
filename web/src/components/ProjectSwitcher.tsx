import { useState, type FormEvent } from 'react'
import { useMutation } from 'urql'
import { graphql } from '../gql'
import { Icon } from './Icon'

const CreateProjectMutation = graphql(`
  mutation CreateProject($name: String!, $repoPath: String!) {
    createProject(name: $name, repoPath: $repoPath) {
      id
    }
  }
`)

/** A form that points the board at a git repository on this computer */
export function NewProjectForm({ onCreated, onCancel }: { onCreated: (id: string) => void; onCancel?: () => void }) {
  const [name, setName] = useState('')
  const [repoPath, setRepoPath] = useState('')
  const [result, createProject] = useMutation(CreateProjectMutation)

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    const res = await createProject({ name, repoPath })
    if (res.data) onCreated(res.data.createProject.id)
  }

  return (
    <form className="new-project" onSubmit={handleSubmit}>
      <label>
        Project name
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="My app" autoFocus />
      </label>
      <label>
        Git repository on this computer
        <input
          className="mono"
          value={repoPath}
          onChange={(e) => setRepoPath(e.target.value)}
          placeholder="/home/you/projects/my-app"
        />
      </label>
      <p className="hint">
        Agents start from the branch that is checked out there, and approved work is merged into it.
      </p>
      {result.error && (
        <p className="error" role="alert">
          {result.error.graphQLErrors[0]?.message ?? result.error.message}
        </p>
      )}
      <div className="form-buttons">
        <button className="primary" disabled={result.fetching || !name.trim() || !repoPath.trim()}>
          Create project
        </button>
        {onCancel && (
          <button type="button" onClick={onCancel}>
            Cancel
          </button>
        )}
      </div>
    </form>
  )
}

type Props = {
  projects: { id: string; name: string; tasks: unknown[] }[]
  selectedId: string
  onSelect: (id: string) => void
}

export function ProjectSwitcher({ projects, selectedId, onSelect }: Props) {
  const [adding, setAdding] = useState(false)

  return (
    <div className="project-switcher">
      <label className="field inline">
        Project
        <select value={selectedId} onChange={(e) => onSelect(e.target.value)}>
          {projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name} ({p.tasks.length})
            </option>
          ))}
        </select>
      </label>
      <button className="icon-button" aria-label="New project" title="New project" onClick={() => setAdding(true)}>
        <Icon name="plus" size={16} />
      </button>

      {adding && (
        <div className="popover" role="dialog" aria-label="New project">
          <NewProjectForm
            onCancel={() => setAdding(false)}
            onCreated={(id) => {
              setAdding(false)
              onSelect(id)
            }}
          />
        </div>
      )}
    </div>
  )
}
