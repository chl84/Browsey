import { writable } from 'svelte/store'

const toastStore = writable<string | null>(null)
let timer: ReturnType<typeof setTimeout> | null = null

export const showToast = (message: string, durationMs = 2000) => {
  toastStore.set(message)
  if (timer) {
    clearTimeout(timer)
  }
  timer = setTimeout(() => {
    toastStore.set(null)
    timer = null
  }, durationMs)
}

export const toast = toastStore
