export const getCurrentWebview = async () => ({
  onDragDropEvent: async (_handler: (event: unknown) => void) => {
    return () => {}
  },
})
