import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createClient as createWSClient } from 'graphql-ws'
import { Client, Provider, cacheExchange, fetchExchange, subscriptionExchange } from 'urql'
import './index.css'
import App from './App.tsx'

// One WebSocket connection, shared by every subscription on the page
const wsClient = createWSClient({
  url: `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/graphql/ws`,
})

const client = new Client({
  url: '/graphql',
  exchanges: [
    cacheExchange,
    fetchExchange,
    // Queries and mutations go over HTTP (fetchExchange); subscriptions go over the WebSocket
    subscriptionExchange({
      forwardSubscription(request) {
        const input = { ...request, query: request.query || '' }
        return {
          subscribe(sink) {
            const unsubscribe = wsClient.subscribe(input, sink)
            return { unsubscribe }
          },
        }
      },
    }),
  ],
  preferGetMethod: false, // our backend runs queries on POST only (GET serves GraphiQL)
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Provider value={client}>
      <App />
    </Provider>
  </StrictMode>,
)
