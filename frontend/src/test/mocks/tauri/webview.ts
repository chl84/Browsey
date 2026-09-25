export const getCurrentWebview = async () => ({
  onDragDropEvent: async (handler: (event: unknown) => void) => {
    const listener = (event: Event) => handler({ payload: (event as CustomEvent).detail })
    window.addEventListener('browsey-e2e-native-drop', listener)
    return () => window.removeEventListener('browsey-e2e-native-drop', listener)
  },
})
