import { get, writable } from 'svelte/store'
import { getErrorMessage, normalizeError } from '@/shared/lib/error'
import type { Partition } from '../model/types'
import {
  canFormatPartition, formatRemovablePartition, getRemovableUsbFormatInfo, usbVolumeLabelError,
  type UsbFilesystem, type UsbFormatInfo, type UsbFormatProgress, type UsbFormatResult,
} from '../services/drives.service'

export type UsbFormatState = {
  target: Partition | null
  filesystem: UsbFilesystem
  label: string
  info: UsbFormatInfo | null
  result: UsbFormatResult | null
  progress: UsbFormatProgress | null
  error: string
  busy: boolean
}

type Deps = {
  loadPartitions: (options: { forceNetworkRefresh: boolean }) => Promise<void>
  reloadCurrent: () => Promise<void>
  openPath: (path: string) => void
  showToast: (message: string) => void
}

export const createUsbFormatModal = (deps: Deps) => {
  const state = writable<UsbFormatState>({
    target: null, filesystem: 'exfat', label: '', info: null, result: null,
    progress: null, error: '', busy: false,
  })
  let request = 0

  const close = () => {
    if (get(state).busy) return
    ++request
    state.update(s => ({ ...s, target: null, info: null, result: null, error: '', progress: null }))
  }

  const open = async (target: Partition) => {
    if (!canFormatPartition(target) || get(state).busy) return
    const token = ++request
    state.update(s => ({ ...s, target, info: null, result: null, error: '', label: '', progress: null }))
    try {
      const info = await getRemovableUsbFormatInfo(target.path)
      if (token !== request) return
      state.update(s => ({ ...s, info, filesystem: info.filesystems.find(option => option.available)?.id ?? s.filesystem }))
    } catch (error) {
      if (token === request) state.update(s => ({ ...s, error: `USB inspection failed: ${getErrorMessage(error)}` }))
    }
  }

  const confirm = async () => {
    const current = get(state)
    if (!current.target || !current.info || current.busy || current.result) return
    const option = current.info.filesystems.find(item => item.id === current.filesystem)
    if (!option?.available || usbVolumeLabelError(current.label, option)) return
    const token = request
    state.update(s => ({ ...s, busy: true, error: '', progress: { phase: 'Checking USB drive', percent: null } }))
    try {
      const result = await formatRemovablePartition(current.target.path, current.filesystem, current.label, progress => {
        if (token === request && get(state).busy) state.update(s => ({ ...s, progress }))
      })
      state.update(s => ({ ...s, result }))
      deps.showToast(`Formatted ${current.target.label} as ${result.filesystem}`)
    } catch (value) {
      const error = normalizeError(value)
      const heading = error.code === 'format_status_unknown' ? 'Formatting status unknown'
        : error.code === 'format_busy' ? 'USB drive busy' : 'Format failed'
      // A failed mount can follow a successful erase. Require fresh inspection
      // before permitting another destructive request, even after a lost reply.
      state.update(s => ({ ...s, info: null, error: `${heading}: ${error.message}` }))
    } finally {
      // Listing refresh is not formatting: it cannot change the erase outcome
      // or suppress reattaching watches invalidated by the unmount.
      for (const refresh of [
        () => deps.loadPartitions({ forceNetworkRefresh: true }), deps.reloadCurrent,
      ]) {
        try { await refresh() } catch {
          deps.showToast('Could not refresh the drive listing. Press F5 to refresh.')
        }
      }
      state.update(s => ({ ...s, busy: false, progress: null }))
    }
  }

  return {
    state, open, close, confirm,
    retry: () => { const target = get(state).target; if (target) return open(target) },
    openResult: () => {
      const current = get(state)
      if (current.busy || !current.result) return
      if (current.result.mountPath) deps.openPath(current.result.mountPath)
      close()
    },
  }
}
