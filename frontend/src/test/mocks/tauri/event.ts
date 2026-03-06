export type UnlistenFn = () => void | Promise<void>

type EventHandler<T = unknown> = (event: { payload: T }) => void

const listeners = new Map<string, Set<EventHandler>>()

export const listen = async <T>(
  eventName: string,
  handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> => {
  const set = listeners.get(eventName) ?? new Set<EventHandler>()
  set.add(handler as EventHandler)
  listeners.set(eventName, set)
  return () => {
    const current = listeners.get(eventName)
    if (!current) return
    current.delete(handler as EventHandler)
    if (current.size === 0) {
      listeners.delete(eventName)
    }
  }
}

export const emitMockEvent = <T>(eventName: string, payload: T) => {
  const handlers = listeners.get(eventName)
  if (!handlers) return
  for (const handler of handlers) {
    handler({ payload })
  }
}
