type WindowAction = 'minimize' | 'toggleMaximize' | 'close'

const runWindowAction = async (action: WindowAction) => {
  const control = (globalThis as {
    __BROWSEY_E2E__?: { windowActions?: WindowAction[]; windowActionFailures?: WindowAction[] }
  }).__BROWSEY_E2E__
  control?.windowActions?.push(action)
  if (control?.windowActionFailures?.includes(action)) {
    throw new Error(`Simulated ${action} failure`)
  }
}

export const getCurrentWindow = () => ({
  minimize: () => runWindowAction('minimize'),
  toggleMaximize: () => runWindowAction('toggleMaximize'),
  close: () => runWindowAction('close'),
})
