export type UnlistenFn = () => void | Promise<void>

export const listen = async <T>(
  _eventName: string,
  _handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> => {
  return () => {}
}
