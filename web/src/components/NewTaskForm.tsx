import { useState, type FormEvent } from 'react'
import { useMutation } from 'urql'
import { graphql } from '../gql'
import type { AgentKind } from '../gql/graphql'

const CreateTaskMutation = graphql(`
  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!, $agent: AgentKind) {
    createTask(projectId: $projectId, title: $title, prompt: $prompt, agent: $agent) {
      id
    }
  }
`)

export type AgentOption = { kind: AgentKind; label: string; ready: boolean; reason?: string | null }

export function NewTaskForm({ projectId, agents }: { projectId: string; agents: AgentOption[] }) {
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [agent, setAgent] = useState<AgentKind>('FAKE')
  const [result, createTask] = useMutation(CreateTaskMutation)

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault() // stop the browser from reloading the page
    if (!title.trim()) return

    // The API calls it prompt: the description is what the agent reads
    const res = await createTask({ projectId, title, prompt: description, agent })
    if (!res.error) {
      setTitle('')
      setDescription('')
    }
  }

  return (
    <form className="new-task" onSubmit={handleSubmit}>
      <input
        className="title-input"
        placeholder="Task title"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
      />
      <input
        className="description-input"
        placeholder="Description: what should the agent do?"
        value={description}
        onChange={(e) => setDescription(e.target.value)}
      />
      <select
        aria-label="Agent"
        value={agent}
        onChange={(e) => setAgent(e.target.value as AgentKind)}
      >
        {agents.map((a) => (
          // An agent that can't run here is listed, but can't be picked
          <option key={a.kind} value={a.kind} disabled={!a.ready}>
            {a.ready ? a.label : `${a.label} (${a.reason})`}
          </option>
        ))}
      </select>
      <button className="primary" disabled={result.fetching || !title.trim()}>
        Add task
      </button>
      {result.error && <span className="error">{result.error.message}</span>}
    </form>
  )
}
