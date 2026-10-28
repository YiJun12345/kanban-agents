import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Client, Provider, cacheExchange, fetchExchange } from 'urql'
import './index.css'
import App from './App.tsx'

const client = new Client({
  url: '/graphql',
  exchanges: [cacheExchange, fetchExchange],
  preferGetMethod: false, // our backend runs queries on POST only (GET serves GraphiQL)
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Provider value={client}>
      <App />
    </Provider>
  </StrictMode>,
)
