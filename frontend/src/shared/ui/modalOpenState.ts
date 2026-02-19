import { derived, writable } from 'svelte/store'

const openModalCount = writable(0)

const incrementModalCount = () => {
  openModalCount.update((count) => count + 1)
}

const decrementModalCount = () => {
  openModalCount.update((count) => Math.max(0, count - 1))
}

export const anyModalOpen = derived(openModalCount, (count) => count > 0)

export const modalOpenState = {
  enter: incrementModalCount,
  leave: decrementModalCount,
}

