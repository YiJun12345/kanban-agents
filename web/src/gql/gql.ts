/* eslint-disable */
import * as types from './graphql';
import type { TypedDocumentNode as DocumentNode } from '@graphql-typed-document-node/core';

/**
 * Map of all GraphQL operations in the project.
 *
 * This map has several performance disadvantages:
 * 1. It is not tree-shakeable, so it will include all operations in the project.
 * 2. It is not minifiable, so the string of a GraphQL query will be multiple times inside the bundle.
 * 3. It does not support dead code elimination, so it will add unused operations.
 *
 * Therefore it is highly recommended to use the babel or swc plugin for production.
 * Learn more about it here: https://the-guild.dev/graphql/codegen/plugins/presets/preset-client#reducing-bundle-size
 */
type Documents = {
    "\n  query Board {\n    projects {\n      id\n      name\n      repoPath\n      defaultBranch\n      tasks {\n        id\n        title\n        prompt\n        status\n        agent\n        updatedAt\n        headline {\n          actor\n          kind\n          author\n          message\n        }\n        claim {\n          quietSecs\n          worker {\n            name\n            builtin\n          }\n        }\n      }\n    }\n    agents {\n      kind\n      label\n      ready\n      reason\n    }\n  }\n": typeof types.BoardDocument,
    "\n  subscription TaskUpdated($projectId: UUID!) {\n    taskUpdated(projectId: $projectId) {\n      id\n    }\n  }\n": typeof types.TaskUpdatedDocument,
    "\n  mutation MoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n": typeof types.MoveTaskDocument,
    "\n  mutation AddJournalNote($taskId: UUID!, $message: String!) {\n    addJournalNote(taskId: $taskId, message: $message) {\n      id\n    }\n  }\n": typeof types.AddJournalNoteDocument,
    "\n  query Logbook($projectId: UUID!) {\n    project(id: $projectId) {\n      id\n      journal(limit: 200) {\n        id\n        seq\n        runId\n        actor\n        kind\n        noteKind\n        author\n        message\n        details\n        createdAt\n        task {\n          id\n          title\n        }\n      }\n    }\n  }\n": typeof types.LogbookDocument,
    "\n  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!, $agent: AgentKind) {\n    createTask(projectId: $projectId, title: $title, prompt: $prompt, agent: $agent) {\n      id\n    }\n  }\n": typeof types.CreateTaskDocument,
    "\n  mutation CreateProject($name: String!, $repoPath: String!) {\n    createProject(name: $name, repoPath: $repoPath) {\n      id\n    }\n  }\n": typeof types.CreateProjectDocument,
    "\n  mutation PanelMoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n": typeof types.PanelMoveTaskDocument,
    "\n  mutation ApproveTask($id: UUID!) {\n    approveTask(id: $id) {\n      id\n      status\n      mergeCommit\n    }\n  }\n": typeof types.ApproveTaskDocument,
    "\n  mutation RequestChanges($id: UUID!, $feedback: String!) {\n    requestChanges(id: $id, feedback: $feedback) {\n      id\n      status\n    }\n  }\n": typeof types.RequestChangesDocument,
    "\n  query TaskPanel($id: UUID!) {\n    task(id: $id) {\n      id\n      title\n      status\n      prompt\n      agent\n      branch\n      mergeCommit\n      claim {\n        quietSecs\n        worker {\n          name\n          builtin\n        }\n      }\n      updatedAt\n      diff\n      logs {\n        runId\n        seq\n        content\n      }\n      journal {\n        id\n        seq\n        runId\n        actor\n        kind\n        noteKind\n        author\n        message\n        details\n        createdAt\n      }\n    }\n  }\n": typeof types.TaskPanelDocument,
    "\n  subscription TaskLogsLive($taskId: UUID!) {\n    taskLogs(taskId: $taskId) {\n      runId\n      seq\n      content\n    }\n  }\n": typeof types.TaskLogsLiveDocument,
    "\n  query Workers {\n    workers {\n      id\n      name\n      builtin\n      lastSeenAt\n      claims {\n        quietSecs\n        task {\n          id\n          title\n        }\n      }\n    }\n  }\n": typeof types.WorkersDocument,
    "\n  mutation RegisterWorker($name: String!) {\n    registerWorker(name: $name) {\n      token\n      worker {\n        id\n        name\n      }\n    }\n  }\n": typeof types.RegisterWorkerDocument,
    "\n  subscription JournalLive($taskId: UUID!) {\n    journalAdded(taskId: $taskId) {\n      id\n      seq\n      runId\n      actor\n      kind\n      noteKind\n      author\n      message\n      details\n      createdAt\n    }\n  }\n": typeof types.JournalLiveDocument,
};
const documents: Documents = {
    "\n  query Board {\n    projects {\n      id\n      name\n      repoPath\n      defaultBranch\n      tasks {\n        id\n        title\n        prompt\n        status\n        agent\n        updatedAt\n        headline {\n          actor\n          kind\n          author\n          message\n        }\n        claim {\n          quietSecs\n          worker {\n            name\n            builtin\n          }\n        }\n      }\n    }\n    agents {\n      kind\n      label\n      ready\n      reason\n    }\n  }\n": types.BoardDocument,
    "\n  subscription TaskUpdated($projectId: UUID!) {\n    taskUpdated(projectId: $projectId) {\n      id\n    }\n  }\n": types.TaskUpdatedDocument,
    "\n  mutation MoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n": types.MoveTaskDocument,
    "\n  mutation AddJournalNote($taskId: UUID!, $message: String!) {\n    addJournalNote(taskId: $taskId, message: $message) {\n      id\n    }\n  }\n": types.AddJournalNoteDocument,
    "\n  query Logbook($projectId: UUID!) {\n    project(id: $projectId) {\n      id\n      journal(limit: 200) {\n        id\n        seq\n        runId\n        actor\n        kind\n        noteKind\n        author\n        message\n        details\n        createdAt\n        task {\n          id\n          title\n        }\n      }\n    }\n  }\n": types.LogbookDocument,
    "\n  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!, $agent: AgentKind) {\n    createTask(projectId: $projectId, title: $title, prompt: $prompt, agent: $agent) {\n      id\n    }\n  }\n": types.CreateTaskDocument,
    "\n  mutation CreateProject($name: String!, $repoPath: String!) {\n    createProject(name: $name, repoPath: $repoPath) {\n      id\n    }\n  }\n": types.CreateProjectDocument,
    "\n  mutation PanelMoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n": types.PanelMoveTaskDocument,
    "\n  mutation ApproveTask($id: UUID!) {\n    approveTask(id: $id) {\n      id\n      status\n      mergeCommit\n    }\n  }\n": types.ApproveTaskDocument,
    "\n  mutation RequestChanges($id: UUID!, $feedback: String!) {\n    requestChanges(id: $id, feedback: $feedback) {\n      id\n      status\n    }\n  }\n": types.RequestChangesDocument,
    "\n  query TaskPanel($id: UUID!) {\n    task(id: $id) {\n      id\n      title\n      status\n      prompt\n      agent\n      branch\n      mergeCommit\n      claim {\n        quietSecs\n        worker {\n          name\n          builtin\n        }\n      }\n      updatedAt\n      diff\n      logs {\n        runId\n        seq\n        content\n      }\n      journal {\n        id\n        seq\n        runId\n        actor\n        kind\n        noteKind\n        author\n        message\n        details\n        createdAt\n      }\n    }\n  }\n": types.TaskPanelDocument,
    "\n  subscription TaskLogsLive($taskId: UUID!) {\n    taskLogs(taskId: $taskId) {\n      runId\n      seq\n      content\n    }\n  }\n": types.TaskLogsLiveDocument,
    "\n  query Workers {\n    workers {\n      id\n      name\n      builtin\n      lastSeenAt\n      claims {\n        quietSecs\n        task {\n          id\n          title\n        }\n      }\n    }\n  }\n": types.WorkersDocument,
    "\n  mutation RegisterWorker($name: String!) {\n    registerWorker(name: $name) {\n      token\n      worker {\n        id\n        name\n      }\n    }\n  }\n": types.RegisterWorkerDocument,
    "\n  subscription JournalLive($taskId: UUID!) {\n    journalAdded(taskId: $taskId) {\n      id\n      seq\n      runId\n      actor\n      kind\n      noteKind\n      author\n      message\n      details\n      createdAt\n    }\n  }\n": types.JournalLiveDocument,
};

/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 *
 *
 * @example
 * ```ts
 * const query = graphql(`query GetUser($id: ID!) { user(id: $id) { name } }`);
 * ```
 *
 * The query argument is unknown!
 * Please regenerate the types.
 */
export function graphql(source: string): unknown;

/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Board {\n    projects {\n      id\n      name\n      repoPath\n      defaultBranch\n      tasks {\n        id\n        title\n        prompt\n        status\n        agent\n        updatedAt\n        headline {\n          actor\n          kind\n          author\n          message\n        }\n        claim {\n          quietSecs\n          worker {\n            name\n            builtin\n          }\n        }\n      }\n    }\n    agents {\n      kind\n      label\n      ready\n      reason\n    }\n  }\n"): (typeof documents)["\n  query Board {\n    projects {\n      id\n      name\n      repoPath\n      defaultBranch\n      tasks {\n        id\n        title\n        prompt\n        status\n        agent\n        updatedAt\n        headline {\n          actor\n          kind\n          author\n          message\n        }\n        claim {\n          quietSecs\n          worker {\n            name\n            builtin\n          }\n        }\n      }\n    }\n    agents {\n      kind\n      label\n      ready\n      reason\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  subscription TaskUpdated($projectId: UUID!) {\n    taskUpdated(projectId: $projectId) {\n      id\n    }\n  }\n"): (typeof documents)["\n  subscription TaskUpdated($projectId: UUID!) {\n    taskUpdated(projectId: $projectId) {\n      id\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation MoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n"): (typeof documents)["\n  mutation MoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation AddJournalNote($taskId: UUID!, $message: String!) {\n    addJournalNote(taskId: $taskId, message: $message) {\n      id\n    }\n  }\n"): (typeof documents)["\n  mutation AddJournalNote($taskId: UUID!, $message: String!) {\n    addJournalNote(taskId: $taskId, message: $message) {\n      id\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Logbook($projectId: UUID!) {\n    project(id: $projectId) {\n      id\n      journal(limit: 200) {\n        id\n        seq\n        runId\n        actor\n        kind\n        noteKind\n        author\n        message\n        details\n        createdAt\n        task {\n          id\n          title\n        }\n      }\n    }\n  }\n"): (typeof documents)["\n  query Logbook($projectId: UUID!) {\n    project(id: $projectId) {\n      id\n      journal(limit: 200) {\n        id\n        seq\n        runId\n        actor\n        kind\n        noteKind\n        author\n        message\n        details\n        createdAt\n        task {\n          id\n          title\n        }\n      }\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!, $agent: AgentKind) {\n    createTask(projectId: $projectId, title: $title, prompt: $prompt, agent: $agent) {\n      id\n    }\n  }\n"): (typeof documents)["\n  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!, $agent: AgentKind) {\n    createTask(projectId: $projectId, title: $title, prompt: $prompt, agent: $agent) {\n      id\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation CreateProject($name: String!, $repoPath: String!) {\n    createProject(name: $name, repoPath: $repoPath) {\n      id\n    }\n  }\n"): (typeof documents)["\n  mutation CreateProject($name: String!, $repoPath: String!) {\n    createProject(name: $name, repoPath: $repoPath) {\n      id\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation PanelMoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n"): (typeof documents)["\n  mutation PanelMoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation ApproveTask($id: UUID!) {\n    approveTask(id: $id) {\n      id\n      status\n      mergeCommit\n    }\n  }\n"): (typeof documents)["\n  mutation ApproveTask($id: UUID!) {\n    approveTask(id: $id) {\n      id\n      status\n      mergeCommit\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RequestChanges($id: UUID!, $feedback: String!) {\n    requestChanges(id: $id, feedback: $feedback) {\n      id\n      status\n    }\n  }\n"): (typeof documents)["\n  mutation RequestChanges($id: UUID!, $feedback: String!) {\n    requestChanges(id: $id, feedback: $feedback) {\n      id\n      status\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query TaskPanel($id: UUID!) {\n    task(id: $id) {\n      id\n      title\n      status\n      prompt\n      agent\n      branch\n      mergeCommit\n      claim {\n        quietSecs\n        worker {\n          name\n          builtin\n        }\n      }\n      updatedAt\n      diff\n      logs {\n        runId\n        seq\n        content\n      }\n      journal {\n        id\n        seq\n        runId\n        actor\n        kind\n        noteKind\n        author\n        message\n        details\n        createdAt\n      }\n    }\n  }\n"): (typeof documents)["\n  query TaskPanel($id: UUID!) {\n    task(id: $id) {\n      id\n      title\n      status\n      prompt\n      agent\n      branch\n      mergeCommit\n      claim {\n        quietSecs\n        worker {\n          name\n          builtin\n        }\n      }\n      updatedAt\n      diff\n      logs {\n        runId\n        seq\n        content\n      }\n      journal {\n        id\n        seq\n        runId\n        actor\n        kind\n        noteKind\n        author\n        message\n        details\n        createdAt\n      }\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  subscription TaskLogsLive($taskId: UUID!) {\n    taskLogs(taskId: $taskId) {\n      runId\n      seq\n      content\n    }\n  }\n"): (typeof documents)["\n  subscription TaskLogsLive($taskId: UUID!) {\n    taskLogs(taskId: $taskId) {\n      runId\n      seq\n      content\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Workers {\n    workers {\n      id\n      name\n      builtin\n      lastSeenAt\n      claims {\n        quietSecs\n        task {\n          id\n          title\n        }\n      }\n    }\n  }\n"): (typeof documents)["\n  query Workers {\n    workers {\n      id\n      name\n      builtin\n      lastSeenAt\n      claims {\n        quietSecs\n        task {\n          id\n          title\n        }\n      }\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation RegisterWorker($name: String!) {\n    registerWorker(name: $name) {\n      token\n      worker {\n        id\n        name\n      }\n    }\n  }\n"): (typeof documents)["\n  mutation RegisterWorker($name: String!) {\n    registerWorker(name: $name) {\n      token\n      worker {\n        id\n        name\n      }\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  subscription JournalLive($taskId: UUID!) {\n    journalAdded(taskId: $taskId) {\n      id\n      seq\n      runId\n      actor\n      kind\n      noteKind\n      author\n      message\n      details\n      createdAt\n    }\n  }\n"): (typeof documents)["\n  subscription JournalLive($taskId: UUID!) {\n    journalAdded(taskId: $taskId) {\n      id\n      seq\n      runId\n      actor\n      kind\n      noteKind\n      author\n      message\n      details\n      createdAt\n    }\n  }\n"];

export function graphql(source: string) {
  return (documents as any)[source] ?? {};
}

export type DocumentType<TDocumentNode extends DocumentNode<any, any>> = TDocumentNode extends DocumentNode<  infer TType,  any>  ? TType  : never;