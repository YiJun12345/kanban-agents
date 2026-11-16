import type { CodegenConfig } from '@graphql-codegen/cli'

const config: CodegenConfig = {
  // CODEGEN_SCHEMA lets you point at another backend, e.g. one on a spare port
  schema: process.env.CODEGEN_SCHEMA ?? 'http://localhost:3000/graphql',
  documents: ['src/**/*.{ts,tsx}', '!src/gql/**'],
  ignoreNoDocuments: true,
  generates: {
    './src/gql/': {
      preset: 'client',
      config: {
        useTypeImports: true,
        enumsAsTypes: true,
        scalars: { UUID: 'string', DateTime: 'string', JSON: 'Record<string, unknown>' },
      },
    },
  },
}

export default config
