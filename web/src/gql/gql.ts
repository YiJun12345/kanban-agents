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
    "\n  query Board {\n    projects {\n      id\n      name\n      tasks {\n        id\n        title\n        status\n        agent\n      }\n    }\n  }\n": typeof types.BoardDocument,
    "\n  mutation MoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n": typeof types.MoveTaskDocument,
    "\n  subscription TaskUpdated($projectId: UUID!) {\n    taskUpdated(projectId: $projectId) {\n      id\n    }\n  }\n": typeof types.TaskUpdatedDocument,
    "\n  query TaskLogs($id: UUID!) {\n    task(id: $id) {\n      id\n      title\n      status\n      prompt\n      logs {\n        runId\n        seq\n        content\n      }\n    }\n  }\n": typeof types.TaskLogsDocument,
    "\n  subscription TaskLogsLive($taskId: UUID!) {\n    taskLogs(taskId: $taskId) {\n      runId\n      seq\n      content\n    }\n  }\n": typeof types.TaskLogsLiveDocument,
    "\n  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!) {\n    createTask(projectId: $projectId, title: $title, prompt: $prompt) {\n      id\n    }\n  }\n": typeof types.CreateTaskDocument,
};
const documents: Documents = {
    "\n  query Board {\n    projects {\n      id\n      name\n      tasks {\n        id\n        title\n        status\n        agent\n      }\n    }\n  }\n": types.BoardDocument,
    "\n  mutation MoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n": types.MoveTaskDocument,
    "\n  subscription TaskUpdated($projectId: UUID!) {\n    taskUpdated(projectId: $projectId) {\n      id\n    }\n  }\n": types.TaskUpdatedDocument,
    "\n  query TaskLogs($id: UUID!) {\n    task(id: $id) {\n      id\n      title\n      status\n      prompt\n      logs {\n        runId\n        seq\n        content\n      }\n    }\n  }\n": types.TaskLogsDocument,
    "\n  subscription TaskLogsLive($taskId: UUID!) {\n    taskLogs(taskId: $taskId) {\n      runId\n      seq\n      content\n    }\n  }\n": types.TaskLogsLiveDocument,
    "\n  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!) {\n    createTask(projectId: $projectId, title: $title, prompt: $prompt) {\n      id\n    }\n  }\n": types.CreateTaskDocument,
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
export function graphql(source: "\n  query Board {\n    projects {\n      id\n      name\n      tasks {\n        id\n        title\n        status\n        agent\n      }\n    }\n  }\n"): (typeof documents)["\n  query Board {\n    projects {\n      id\n      name\n      tasks {\n        id\n        title\n        status\n        agent\n      }\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation MoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n"): (typeof documents)["\n  mutation MoveTask($id: UUID!, $status: TaskStatus!) {\n    moveTask(id: $id, status: $status) {\n      id\n      status\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  subscription TaskUpdated($projectId: UUID!) {\n    taskUpdated(projectId: $projectId) {\n      id\n    }\n  }\n"): (typeof documents)["\n  subscription TaskUpdated($projectId: UUID!) {\n    taskUpdated(projectId: $projectId) {\n      id\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query TaskLogs($id: UUID!) {\n    task(id: $id) {\n      id\n      title\n      status\n      prompt\n      logs {\n        runId\n        seq\n        content\n      }\n    }\n  }\n"): (typeof documents)["\n  query TaskLogs($id: UUID!) {\n    task(id: $id) {\n      id\n      title\n      status\n      prompt\n      logs {\n        runId\n        seq\n        content\n      }\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  subscription TaskLogsLive($taskId: UUID!) {\n    taskLogs(taskId: $taskId) {\n      runId\n      seq\n      content\n    }\n  }\n"): (typeof documents)["\n  subscription TaskLogsLive($taskId: UUID!) {\n    taskLogs(taskId: $taskId) {\n      runId\n      seq\n      content\n    }\n  }\n"];
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!) {\n    createTask(projectId: $projectId, title: $title, prompt: $prompt) {\n      id\n    }\n  }\n"): (typeof documents)["\n  mutation CreateTask($projectId: UUID!, $title: String!, $prompt: String!) {\n    createTask(projectId: $projectId, title: $title, prompt: $prompt) {\n      id\n    }\n  }\n"];

export function graphql(source: string) {
  return (documents as any)[source] ?? {};
}

export type DocumentType<TDocumentNode extends DocumentNode<any, any>> = TDocumentNode extends DocumentNode<  infer TType,  any>  ? TType  : never;