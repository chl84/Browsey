import {appearance, appearanceManifest, appearanceProbes} from './appearance.mjs'
import {feedback, feedbackManifest} from './feedback.mjs'
import {cancellations, cancellationManifest, cancellationProbes} from './cancellation.mjs'
export const measurementManifest = plan => [...appearanceManifest(plan), ...feedbackManifest(plan), ...cancellationManifest(plan)]
export const measurementProbes = plan => [...appearanceProbes(plan), ...cancellationProbes(plan)]
export async function measurements(plan, fixture, ui, record) {
  measurementManifest(plan)
  await appearance(plan, fixture, ui, record, {measure: true})
  await feedback(plan, fixture, ui, record)
  await cancellations(plan, fixture, ui, record)
}
