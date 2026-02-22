export type WheelAssistOptions = {
  defaultLineHeightPx?: number
  baseGain?: number
  accelerationScale?: number
  accelerationMax?: number
  wheelNotchThresholdPx?: number
  wheelTickBoost?: number
  trackpadBoost?: number
  minWheelStepPx?: number
  maxStepPx?: number
  edgeBoostDistanceRatio?: number
  edgeBoostMax?: number
  burstGapMs?: number
  velocitySmoothing?: number
  velocityGain?: number
  paddingCacheMs?: number
}

const defaultOptions: Required<WheelAssistOptions> = {
  defaultLineHeightPx: 16,
  baseGain: 0.5,
  accelerationScale: 560,
  accelerationMax: 1.3,
  wheelNotchThresholdPx: 8,
  wheelTickBoost: 1.1,
  trackpadBoost: 1,
  minWheelStepPx: 11,
  maxStepPx: 200,
  edgeBoostDistanceRatio: 0.35,
  edgeBoostMax: 1.2,
  burstGapMs: 130,
  velocitySmoothing: 0.62,
  velocityGain: 0.55,
  paddingCacheMs: 250,
}

const explorerWheelAssistConfig: Readonly<Required<WheelAssistOptions>> = Object.freeze({
  ...defaultOptions,
})

type WheelAssistState = {
  nativeBurstUntilTs: number
  lastEventAt: number
  lastDirection: -1 | 0 | 1
  velocityPxPerMs: number
  paddingTop: number
  paddingBottom: number
  paddingAt: number
}

const DOM_DELTA_PIXEL = 0
const DOM_DELTA_LINE = 1
const DOM_DELTA_PAGE = 2

const stateByElement = new WeakMap<HTMLElement, WheelAssistState>()

const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value))
const clampAbs = (value: number, maxAbs: number) => clamp(value, -Math.abs(maxAbs), Math.abs(maxAbs))

const resolveLineHeight = (el: HTMLElement, fallback: number) => {
  const raw = getComputedStyle(el).lineHeight
  const parsed = Number.parseFloat(raw)
  return Number.isFinite(parsed) ? parsed : fallback
}

const getOrCreateState = (el: HTMLElement): WheelAssistState => {
  const existing = stateByElement.get(el)
  if (existing) return existing
  const created: WheelAssistState = {
    nativeBurstUntilTs: 0,
    lastEventAt: 0,
    lastDirection: 0,
    velocityPxPerMs: 0,
    paddingTop: 0,
    paddingBottom: 0,
    paddingAt: 0,
  }
  stateByElement.set(el, created)
  return created
}

const readVerticalPadding = (el: HTMLElement, state: WheelAssistState, now: number, cacheMs: number) => {
  if (state.paddingAt !== 0 && now - state.paddingAt < cacheMs) {
    return { top: state.paddingTop, bottom: state.paddingBottom }
  }
  const styles = getComputedStyle(el)
  const paddingTop = Number.parseFloat(styles.paddingTop)
  const paddingBottom = Number.parseFloat(styles.paddingBottom)
  state.paddingTop = Number.isFinite(paddingTop) ? paddingTop : 0
  state.paddingBottom = Number.isFinite(paddingBottom) ? paddingBottom : 0
  state.paddingAt = now
  return { top: state.paddingTop, bottom: state.paddingBottom }
}

const computeEffectiveEdgeDistance = (
  el: HTMLElement,
  state: WheelAssistState,
  now: number,
  maxScrollTop: number,
  prevTop: number,
  movingUp: boolean,
  cacheMs: number,
) => {
  const { top, bottom } = readVerticalPadding(el, state, now, cacheMs)
  const effectiveTop = clamp(top, 0, maxScrollTop)
  const effectiveBottom = clamp(maxScrollTop - bottom, effectiveTop, maxScrollTop)
  return movingUp
    ? Math.max(0, prevTop - effectiveTop)
    : Math.max(0, effectiveBottom - prevTop)
}

const normalizeWheelDeltaY = (event: WheelEvent, el: HTMLElement, fallbackLineHeight: number) => {
  if (!Number.isFinite(event.deltaY) || event.deltaY === 0) {
    return 0
  }
  if (event.deltaMode === DOM_DELTA_PIXEL) {
    return event.deltaY
  }
  if (event.deltaMode === DOM_DELTA_LINE) {
    return event.deltaY * resolveLineHeight(el, fallbackLineHeight)
  }
  if (event.deltaMode === DOM_DELTA_PAGE) {
    return event.deltaY * el.clientHeight
  }
  return event.deltaY
}

export const applyWheelScrollAssist = (
  el: HTMLElement,
  event: WheelEvent,
) => {
  if (event.defaultPrevented || event.ctrlKey) return false

  const cfg = explorerWheelAssistConfig
  const maxScrollTop = Math.max(0, el.scrollHeight - el.clientHeight)
  if (maxScrollTop <= 0) return false

  const deltaY = normalizeWheelDeltaY(event, el, cfg.defaultLineHeightPx)
  if (deltaY === 0) return false

  const state = getOrCreateState(el)
  const prevEventAt = state.lastEventAt
  const now = Number.isFinite(event.timeStamp) && event.timeStamp > 0 ? event.timeStamp : performance.now()
  const direction: -1 | 1 = deltaY < 0 ? -1 : 1
  const burstExpired = prevEventAt === 0 || now - prevEventAt > cfg.burstGapMs
  const directionChanged = state.lastDirection !== 0 && state.lastDirection !== direction
  if (burstExpired || directionChanged) {
    state.nativeBurstUntilTs = 0
    state.velocityPxPerMs = 0
  }

  if (state.nativeBurstUntilTs > now) {
    state.lastEventAt = now
    state.lastDirection = direction
    return false
  }

  if (!event.cancelable) {
    // Keep the rest of the burst native after a non-cancelable event.
    state.nativeBurstUntilTs = now + cfg.burstGapMs
    state.lastEventAt = now
    state.lastDirection = direction
    return false
  }

  state.lastEventAt = now
  state.lastDirection = direction

  const dt = clamp(now - prevEventAt, 1, 80)
  const instantVelocity = Math.abs(deltaY) / dt
  state.velocityPxPerMs =
    state.velocityPxPerMs * cfg.velocitySmoothing + instantVelocity * (1 - cfg.velocitySmoothing)

  const absDelta = Math.abs(deltaY)
  const isLikelyWheel = event.deltaMode !== DOM_DELTA_PIXEL || absDelta >= cfg.wheelNotchThresholdPx
  const shouldEnforceMinStep = event.deltaMode === DOM_DELTA_LINE || event.deltaMode === DOM_DELTA_PAGE
  const inputBoost = isLikelyWheel ? cfg.wheelTickBoost : cfg.trackpadBoost
  const magnitudeGain = cfg.baseGain + absDelta / Math.max(1, cfg.accelerationScale)
  const burstGain = 1 + state.velocityPxPerMs * cfg.velocityGain
  const rawGain = magnitudeGain * burstGain * inputBoost
  const baseGain = clamp(rawGain, cfg.baseGain, cfg.accelerationMax)
  const prevTop = el.scrollTop
  const movingUp = deltaY < 0
  const edgeDistance = computeEffectiveEdgeDistance(
    el,
    state,
    now,
    maxScrollTop,
    prevTop,
    movingUp,
    cfg.paddingCacheMs,
  )
  const edgeBoostDistance = Math.max(24, el.clientHeight * cfg.edgeBoostDistanceRatio)
  const edgeRatio = clamp(1 - edgeDistance / edgeBoostDistance, 0, 1)
  const edgeGain = 1 + edgeRatio * (cfg.edgeBoostMax - 1)
  const gain = Math.min(cfg.accelerationMax, baseGain * edgeGain)
  const minWheelStepPx = cfg.minWheelStepPx
  const maxStepPx = cfg.maxStepPx
  let step = deltaY * gain
  if (isLikelyWheel && shouldEnforceMinStep && Math.abs(step) < minWheelStepPx) {
    step = Math.sign(deltaY) * minWheelStepPx
  }
  step = clampAbs(step, maxStepPx)
  if (step === 0) return false

  const nextTop = clamp(Math.round(prevTop + step), 0, maxScrollTop)
  if (nextTop === prevTop) {
    // Keep the whole burst in custom mode instead of silently bouncing to native.
    event.preventDefault()
    return true
  }

  el.scrollTop = nextTop
  event.preventDefault()
  return true
}
