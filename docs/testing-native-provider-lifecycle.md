# Provider lifecycle mode: approval and isolation contract

Defined for NT5-5, 2026-10-06. This is a reviewable specification, not an enabled
runner or evidence of real device lifecycle acceptance. Mounted-folder NT5
success establishes file operations only. The foundation guard and its discovery,
mount, clipboard, trash, formatting and external-launch restrictions stay intact.

## Approval before execution

A future lifecycle runner must require a separate maintainer approval for one
scenario, one exact device/service identity and one reviewed action timeline.
Approval of an `ai_agent_testfolder` alone does not authorize disrupting a device,
mount, connection or phone used by other applications. The current NT5 instruction
covers defining this contract, not performing those disruptions.

The private mode-600 approval manifest must bind its schema, scenario, expiry,
maintainer approval, fresh run UUID/nonce, candidate/source/harness hashes, exact
approved folder/URI, exact mount/device identity, private credential path and the
SHA-256 of the canonical action plan. No account, device-root, mount-root or camera
inventory may be used to find a target. Connection identity must be supplied;
reading credentials, signaling a service and changing desktop/global config must
not be inferred from folder approval. Credentials and device/account identifiers
remain private and are never committed with this specification.

A reviewed scope includes at most one provider plus mandatory local ownership
anchor, one generated source/destination pair and unrelated generated sentinels.
Files remain at most 64 KiB; no large-file stress, formatting, global trash or
system clipboard is enabled. Ordinary tests never accept a lifecycle manifest
as authority to broaden commands.

## Scenario contracts

| Scenario | Separate authorized action | Required evidence |
| --- | --- | --- |
| First connection | Connect exactly the supplied disposable service/device; do not discover alternatives | Exact identity and approved folder become accessible; generated listing and transfer work; missing roots are not created or substituted |
| Locked phone | Maintainer locks/unlocks the designated test phone, or supplies a reviewed device-only controller | Truthful unavailable/locked feedback; no successful empty-folder claim; source bytes survive; exact approved folder recovers after unlock |
| Reconnect | Disconnect/reconnect the exact reviewed connection | Existing operation settles before reconnect; failure/partial counts are truthful; no mutation replay; explicit fresh navigation works after reconnect |
| Disappearing mount | Remove only a dedicated test mount or isolated service instance | No fallback into an empty local mountpoint, personal home or another provider; stale paths fail; generated sources/uncertain output remain retained |
| Busy/ejected media | Attempt one reviewed eject of a dedicated test medium with a declared owned open handle | Busy failure is shown without falsely claiming eject; a later separately declared eject uses the exact medium and preserves sources; no unrelated processes are killed |

First-connection and device-state cases need an approved disposable target or a
maintainer-controlled action. A namespace mount/service can test its defined
fault semantics but does not certify physical USB or locked-MTP behavior. A fake
UI event, mocked IPC, stopping only the candidate or renaming a fixture does not
prove a real mount/device lifecycle transition.

## Guard and runner requirements

The future runner must implement a separate `provider-lifecycle` entry point.
Default foundation sessions reject every lifecycle command. Only the exact
reviewed action receives a one-use exception, after validating identity and plan
hash before any discovery, connection, signal or mutation I/O. Unknown commands,
changed identities, extra providers, outside paths and omitted approvals fail
closed. A broad `allowMounts`/`allowDiscovery` boolean is insufficient.

The approved directory must already exist on the designated provider. Before a
first connection it may be temporarily inaccessible; after connecting, the
runner validates that exact path and provider identity without creating it or
listing parents. It then creates a fresh exclusive owned UUID subtree. Every
UI file operation, readback and recovery check remains confined there. A
reconnected provider must retain the reviewed identity; a same-spelled replacement
mount is not adopted. Reads/mutations from a previous epoch cannot silently
resume against a new mount identity.

Use the actual separate WebKitGTK candidate, pointer/keyboard controls and owned
window accessibility. The lifecycle transition needs a separate identity-bound
observer in addition to UI feedback. Record action start/acknowledgement, observed
provider-state epoch, actual operation result, partial outputs and independent
source/destination digests. If the transition does not occur, report BLOCKED;
never substitute mounted-folder PASS. Do not hide flakes with operation retries.

Each plan has at most four declared transitions, at most 180 seconds per state
observation and 600 seconds to settle an existing operation, with a 15-minute
whole-scenario checked elapsed budget. These are observation limits, not hard
interrupt guarantees for stalled kernel/device calls. Child CLI calls retain the
existing bounded timeout/retry policy. Retention preflight must fit the current
finite run/byte reservation limits; failure stops before device changes.

## Teardown and recovery

Stop owned UI work, close the WebDriver session, stop only exact owned drivers
and confirm exact candidate/driver PID/start identities gone. Wait for owned
provider helper processes before auditing the private local run. Failure,
uncertain mounts, interrupted output or ambiguous helper ownership means retained
FAIL/BLOCKED evidence and no next scenario until independently reviewed.

Restore only state explicitly covered by the approved action plan and confirmed
identity. Do not reconnect, unlock, mount or eject automatically as cleanup when
that action was not separately declared. Keep the local ownership anchor, private
credentials/report and every remote source/output fixture. No automatic recovery
file deletion, account inventory, global unmount or broad process kill is allowed.

A completed scenario must have native PASS, independent provider-state evidence,
independent byte/metadata preservation, all four owned teardown stages and a fresh
private retention audit. Publish only redacted case IDs and measured scope.
No scenario in this document has been executed by the NT5-5 definition work.
