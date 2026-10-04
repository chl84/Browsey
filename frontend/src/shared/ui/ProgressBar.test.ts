import { mount, tick, unmount } from 'svelte'
import { expect, it } from 'vitest'
import ProgressBar from './ProgressBar.svelte'

it('retains progressbar defaults and supports a descriptive meter without separate styling', async () => {
  const progress = mount(ProgressBar, { target: document.body, props: { percent: 120 } })
  await tick()
  expect(document.querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')).toBe('100')
  await unmount(progress)
  const meter = mount(ProgressBar, {
    target: document.body, props: { role: 'meter', label: 'Used disk space', percent: 25, valueText: '8 GB used of 32 GB' },
  })
  try {
    await tick()
    expect(document.querySelector('[role="meter"]')?.getAttribute('aria-valuetext')).toBe('8 GB used of 32 GB')
  } finally {
    await unmount(meter)
    document.body.innerHTML = ''
  }
})
