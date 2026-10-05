// Some native WebKit input paths report a primary mouseup/click after a
// secondary mousedown/contextmenu. Use the gesture's original button too.
export const createPrimaryClickGuard = () => {
  let pointerButton = 0
  return {
    down: (event: MouseEvent) => { pointerButton = event.button },
    accept: (event: MouseEvent) => {
      const allowed = event.button === 0 && (event.detail === 0 || pointerButton === 0)
      pointerButton = 0
      return allowed
    },
  }
}
