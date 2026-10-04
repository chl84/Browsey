import { describe, expectTypeOf, it } from 'vitest'
import type { ComponentProps } from 'svelte'
import type ExplorerShell from '../ui-shell/components/ExplorerShell.svelte'
import type { Entry, Partition } from '../model/types'
import type { PropertiesState } from '../modals/propertiesModal'
import type { DeleteConfirmState } from '../modals/deleteConfirmModal'
import type { createExplorerShellProps, ExplorerShellParams, ExplorerShellProps } from './createExplorerShellProps'

// These checks are verified by svelte-check as well as Vitest. In particular,
// an accidental `any` would make the negative assignments stop being errors.
const rejectInvalidContracts = (params: ExplorerShellParams, props: ExplorerShellProps) => {
  // @ts-expect-error Entries must include all required domain fields.
  params.visibleEntries = [{ path: '/missing-entry-fields' }]
  // @ts-expect-error A row mouse handler cannot accept a keyboard event.
  params.handleRowsMouseDown(new KeyboardEvent('keydown'))
  // @ts-expect-error Permission toggles only accept supported scopes.
  params.propertiesModal.toggleAccess('everyone', 'read', true)
  // @ts-expect-error Modal state is not an untyped prop bag.
  props.modalProps.openWithLoading = 'yes'
  // @ts-expect-error Required sidebar values cannot be silently omitted.
  props.sidebarProps = {}
}
void rejectInvalidContracts

describe('ExplorerShell prop contracts', () => {
  it('keeps concrete domain values and modal states', () => {
    expectTypeOf<ExplorerShellParams[keyof ExplorerShellParams]>().not.toBeAny()
    expectTypeOf<ExplorerShellParams['visibleEntries']>().toEqualTypeOf<Entry[]>()
    expectTypeOf<ExplorerShellParams['partitions']>().toEqualTypeOf<Partition[]>()
    expectTypeOf<ExplorerShellParams['propertiesState']>().toEqualTypeOf<PropertiesState>()
    expectTypeOf<ExplorerShellParams['deleteState']>().toEqualTypeOf<DeleteConfirmState>()
  })

  it('checks callback arguments at the assembly boundary', () => {
    expectTypeOf<ExplorerShellParams['handleRowsMouseDown']>().parameter(0).toEqualTypeOf<MouseEvent>()
    expectTypeOf<ExplorerShellParams['handleRowContextMenu']>().parameters.toEqualTypeOf<[Entry, MouseEvent]>()
    expectTypeOf<ExplorerShellParams['toggleColumnFilter']>().parameters
      .toEqualTypeOf<['name' | 'type' | 'modified' | 'size', string, boolean]>()
  })

  it('preserves the consuming component contract without any escape hatches', () => {
    type Consumer = ComponentProps<typeof ExplorerShell>
    expectTypeOf<ExplorerShellProps[keyof ExplorerShellProps]>().not.toBeAny()
    expectTypeOf<ReturnType<typeof createExplorerShellProps>>().toEqualTypeOf<ExplorerShellProps>()
    expectTypeOf<ExplorerShellProps['modalProps']>().toEqualTypeOf<Consumer['modalProps']>()
    expectTypeOf<ExplorerShellProps['listingProps']>().toEqualTypeOf<Consumer['listingProps']>()
  })
})
