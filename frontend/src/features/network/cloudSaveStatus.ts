import type { CloudWritebackStatus, CloudSaveStatus } from './cloud.service'

export const cloudSaveLabel = (status: CloudSaveStatus | undefined) => {
  switch (status) {
    case 'saved': return 'Saved to cloud'
    case 'pending': return 'Waiting to save'
    case 'uploading': return 'Saving to cloud…'
    case 'conflict': return 'Conflict — local edits kept'
    case 'error': return 'Save failed — local edits kept'
    case 'paused': return 'Automatic saving paused'
    case 'unsupported': return 'Manual upload required'
    default: return 'Manual saving'
  }
}

// An initial status request can finish after a newer live event. Never display
// its older Saved state over a current conflict or an unfinished upload.
export const mergeCloudSaveStatus = (rows: CloudWritebackStatus[], next: CloudWritebackStatus) => {
  const previous = rows.find(row => row.id === next.id)
  if (previous && previous.sequence >= next.sequence) return rows
  return [...rows.filter(row => row.id !== next.id), next]
}

export const cloudSaveSummary = (rows: CloudWritebackStatus[]) => {
  const attention = rows.filter(row => ['conflict', 'error', 'unsupported', 'paused'].includes(row.status))
  if (attention.length) return `${attention.length} need attention`
  const saving = rows.filter(row => row.status === 'pending' || row.status === 'uploading')
  return saving.length ? `Saving ${saving.length}…` : 'Saved'
}
