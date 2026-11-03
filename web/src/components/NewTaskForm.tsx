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
  const [description, setDescription] = useState('')
  const [result, createTask] = useMutation(CreateTaskMutation)

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault() // stop the browser from reloading the page
    if (!title.trim()) return

    // The API calls it prompt: the description is what the agent reads
    const res = await createTask({ projectId, title, prompt: description })
    if (!res.error) {
      setTitle('')
      setDescription('')
    }
  }

  return (
    <form className="new-task" onSubmit={handleSubmit}>
      <input placeholder="Task title" value={title} onChange={(e) => setTitle(e.target.value)} />
      <input
        className="description-input"
        placeholder="Description: what should the agent do?"
        value={description}
        onChange={(e) => setDescription(e.target.value)}
      />
      <button disabled={result.fetching || !title.trim()}>Add task</button>
      {result.error && <span className="error">{result.error.message}</span>}
    </form>
  )
}
