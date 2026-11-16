import { useEffect, useRef } from 'react'
import type { OperationContext } from 'urql'

/**
 * Keep a query fresh while live events arrive.
 *
 * `tick` is a number that goes up with every live event. After each event we refetch.
 * If a fetch is already on its way, its answer may be older than the event, so we wait
 * for it to finish and then fetch once more. That way the last event is never missed,
 * however fast the events come.
 */
export function useRefetchOnTick(
  tick: number,
  busy: boolean, // is the query fetching right now?
  refetch: (opts?: Partial<OperationContext>) => void,
) {
  const seen = useRef(tick) // the newest tick we have fetched for
  useEffect(() => {
    if (tick !== seen.current && !busy) {
      seen.current = tick
      refetch({ requestPolicy: 'network-only' })
    }
  }, [tick, busy, refetch])
}
