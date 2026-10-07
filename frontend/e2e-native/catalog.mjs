// Shared executable suite registry; no runner/profile initialization on import.
import {foundation, foundationManifest} from './cases.mjs'
import {smoke, smokeManifest} from './tiers.mjs'
import {repeatability, repeatabilityManifest} from './repeatability.mjs'
import {measurements, measurementManifest} from './measurements.mjs'
import {storagePerformance, storageManifest} from './storage-performance.mjs'
import {cloudExport,cloudExportManifest} from './cloud-export.mjs'
import {cloudScale,cloudScaleManifest} from './cloud-scale.mjs'
import {cloudRace,cloudRaceManifest} from './cloud-race.mjs'
import {cloudTrash,cloudTrashManifest} from './cloud-trash.mjs'
import {navigation, navigationManifest} from './navigation.mjs'
import {listing, listingManifest} from './listing.mjs'
import {selection, selectionManifest} from './selection.mjs'
import {creation, creationManifest} from './creation.mjs'
import {names, namesManifest} from './names.mjs'
import {limits, limitsManifest} from './limits.mjs'
import {contents, contentsManifest} from './contents.mjs'
import {trees, treesManifest} from './trees.mjs'
import {links, linksManifest} from './links.mjs'
import {usb, usbManifest, network, networkManifest, mobile, mobileManifest} from './providers.mjs'
import {drag, dragManifest} from './drag.mjs'
import {feedback, feedbackManifest} from './feedback.mjs'
import {openWith, openWithManifest} from './open-with.mjs'
import {archives, archivesManifest} from './archives.mjs'
import {watchers, watchersManifest} from './watchers.mjs'
import {appearance, appearanceManifest} from './appearance.mjs'
import {keyboard, keyboardManifest} from './keyboard.mjs'
import {services, servicesManifest} from './desktop-services.mjs'
import {cloudProvider, cloudManifest, cloudWorking} from './cloud-provider.mjs'
import {editing, editingManifest} from './editing.mjs'
import {hidden, hiddenManifest} from './hidden.mjs'
import {overwrites, overwriteManifest} from './overwrite.mjs'
import {moves, moveManifest} from './moves.mjs'
import {access, accessManifest} from './access.mjs'
import {ioFaults, ioFaultManifest} from './iofaults.mjs'
import {races, raceManifest} from './races.mjs'
import {interruption, interruptionManifest} from './interruption.mjs'
import {cancellations, cancellationManifest} from './cancellation.mjs'
import {transfers, transferManifest} from './transfers.mjs'
import {batches, batchManifest} from './batch.mjs'
import {guards, guardManifest} from './guards.mjs'
import {conflicts, conflictManifest} from './conflicts.mjs'
import {progress, progressManifest} from './progress.mjs'

export const suites = { smoke: {run: smoke, manifest: smokeManifest}, foundation: { run: foundation, manifest: foundationManifest },
  navigation: { run: navigation, manifest: navigationManifest }, listing: { run: listing, manifest: listingManifest },
  selection: { run: selection, manifest: selectionManifest }, creation: { run: creation, manifest: creationManifest } }
suites.measurements = {run: measurements, manifest: measurementManifest}
suites['storage-performance'] = {run: storagePerformance, manifest: storageManifest}
suites['cloud-export'] = {run:cloudExport,manifest:cloudExportManifest}
suites['cloud-scale'] = {run:cloudScale,manifest:cloudScaleManifest}
suites['cloud-race'] = {run:cloudRace,manifest:cloudRaceManifest}
suites['cloud-trash'] = {run:cloudTrash,manifest:cloudTrashManifest}
suites.repeatability = {run: repeatability, manifest: repeatabilityManifest}
for (const group of ['editing', 'fileops', 'rename', 'properties', 'history']) suites[group] = {
  run: (plan, fixture, ui, record) => editing(plan, fixture, ui, record, group),
  manifest: plan => editingManifest(plan, group),
}
suites['transfers-within'] = { run: transfers, manifest: transferManifest }
suites['transfers-hub'] = { run: (plan, fixture, ui, record) => transfers(plan, fixture, ui, record, 'hub'),
  manifest: plan => transferManifest(plan, 'hub') }
suites['transfers-pairs'] = { run: (plan, fixture, ui, record) => transfers(plan, fixture, ui, record, 'pairs'),
  manifest: plan => transferManifest(plan, 'pairs') }
suites['guards-aliases-mobile'] = { run: (plan, fixture, ui, record) => guards(plan, fixture, ui, record, 'remaining'),
  manifest: plan => guardManifest(plan, 'remaining') }
suites.batches = { run: batches, manifest: batchManifest }
suites.guards = { run: guards, manifest: guardManifest }
suites.conflicts = { run: conflicts, manifest: conflictManifest }
suites.progress = { run: progress, manifest: progressManifest }
suites.cancellation = { run: cancellations, manifest: cancellationManifest }
suites.overwrite = { run: overwrites, manifest: overwriteManifest }
suites.moves = { run: moves, manifest: moveManifest }
suites.access = { run: access, manifest: accessManifest }
suites.iofaults = { run: ioFaults, manifest: ioFaultManifest }
suites.races = { run: races, manifest: raceManifest }
suites.interruption = { run: interruption, manifest: interruptionManifest }
suites.names = { run: names, manifest: namesManifest }
suites.hidden = { run: hidden, manifest: hiddenManifest }
suites.limits = { run: limits, manifest: limitsManifest }
suites.contents = { run: contents, manifest: contentsManifest }
suites.trees = { run: trees, manifest: treesManifest }
suites.links = { run: links, manifest: linksManifest }
suites['cloud-working'] = { run: cloudWorking, manifest: plan=>cloudManifest(plan,'working') }
suites['cloud-provider'] = { run: cloudProvider, manifest: cloudManifest }
suites.mobile = { run: mobile, manifest: mobileManifest }
suites.network = { run: network, manifest: networkManifest }
suites.drag = {run:drag,manifest:dragManifest}
suites['drag-feedback'] = {run:feedback,manifest:feedbackManifest}
suites['open-with']={run:openWith,manifest:openWithManifest}
suites.archives={run:archives,manifest:archivesManifest}
suites.watchers={run:watchers,manifest:watchersManifest}
suites.appearance={run:appearance,manifest:appearanceManifest}
suites.keyboard={run:keyboard,manifest:keyboardManifest}
suites['desktop-services'] = {run:services,manifest:servicesManifest}
suites.usb = { run: usb, manifest: usbManifest }
suites['usb-access'] = { run: (plan,fixture,ui,record)=>access(plan,fixture,ui,record,'usb'), manifest: plan=>{usbManifest(plan);return accessManifest(plan,'usb')} }
