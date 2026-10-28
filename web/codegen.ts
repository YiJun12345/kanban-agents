import type { CodegenConfig } from '@graphql-codegen/cli'

const config: CodegenConfig = {
  schema: 'http://localhost:3000/graphql',
  documents: ['src/**/*.tsx'],
  ignoreNoDocuments: true,
  generates: {
    './src/gql/': {
      preset: 'client',
      config: {
        useTypeImports: true,
        enumsAsTypes: true,
        scalars: { UUID: 'string', DateTime: 'string' },
      },
    },
  },
}

export default config
