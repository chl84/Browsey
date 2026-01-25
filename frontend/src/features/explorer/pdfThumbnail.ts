import { getDocument, GlobalWorkerOptions, version as pdfjsVersion, VerbosityLevel } from 'pdfjs-dist'
import workerSrc from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

GlobalWorkerOptions.workerSrc = workerSrc

type RenderResult = {
  blob: Blob
  width: number
  height: number
}

// Render first page to PNG, scaled so the longest side is <= maxDim.
export async function renderPdfFirstPage(bytes: Uint8Array, maxDim: number): Promise<RenderResult> {
  const loadingTask = getDocument({
    data: bytes,
    useSystemFonts: true,
    verbosity: VerbosityLevel.ERRORS,
  })

  const pdf = await loadingTask.promise
  const page = await pdf.getPage(1)

  const viewport = page.getViewport({ scale: 1 })
  const scale = Math.min(1, maxDim / Math.max(viewport.width, viewport.height))
  const scaled = page.getViewport({ scale })

  const { canvas, ctx } = createCanvas(Math.ceil(scaled.width), Math.ceil(scaled.height))

  const canvasContext = ctx as unknown as CanvasRenderingContext2D

  await page.render({
    canvasContext,
    viewport: scaled,
  }).promise

  const blob = await toPngBlob(canvas)

  return {
    blob,
    width: Math.ceil(scaled.width),
    height: Math.ceil(scaled.height),
  }
}

function createCanvas(width: number, height: number) {
  if ('OffscreenCanvas' in globalThis) {
    const canvas = new OffscreenCanvas(width, height)
    const ctx = canvas.getContext('2d', { alpha: true }) as OffscreenCanvasRenderingContext2D | null
    if (!ctx) throw new Error('Failed to get canvas context')
    return { canvas, ctx }
  }

  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  const ctx = canvas.getContext('2d', { alpha: true })
  if (!ctx) throw new Error('Failed to get canvas context')
  return { canvas, ctx }
}

async function toPngBlob(canvas: OffscreenCanvas | HTMLCanvasElement): Promise<Blob> {
  if (canvas instanceof OffscreenCanvas) {
    return canvas.convertToBlob({ type: 'image/png' })
  }

  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (!blob) return reject(new Error('Failed to create PNG blob'))
      resolve(blob)
    }, 'image/png')
  })
}
