import { useState, type FormEvent } from 'react'
import { useMutation } from 'urql'
import { graphql } from '../gql'

const CreateTaskMutation = graphql(`
  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!) {
    createTask(projectId: $projectId, title: $title, prompt: $prompt) {
      id
    }
  }
`)

export function NewTaskForm({ projectId }: { projectId: string }) {
  const [title, setTitle] = useState('')
  const [prompt, setPrompt] = useState('')
  const [result, createTask] = useMutation(CreateTaskMutation)

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault() // stop the browser from reloading the page
    if (!title.trim()) return

    const res = await createTask({ projectId, title, prompt })
    if (!res.error) {
      setTitle('')
      setPrompt('')
    }
  }

  return (
    <form className="new-task" onSubmit={handleSubmit}>
      <input placeholder="Task title" value={title} onChange={(e) => setTitle(e.target.value)} />
      <input
        placeholder="Prompt for the agent"
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
      />
      <button disabled={result.fetching}>Add task</button>
      {result.error && <span className="error">{result.error.message}</span>}
    </form>
  )
}
