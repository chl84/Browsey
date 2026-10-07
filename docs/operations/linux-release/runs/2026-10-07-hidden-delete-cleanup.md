# Properties Hidden changes and MTP cleanup

Date: 2026-10-07. Only generated files below the already approved
`ai_agent_testfolder` roots were changed. Personal data, installed app sessions,
desktop services and system configuration were not used for diagnosis.

## Browsey correction

On Unix, changing Hidden renames an entry. Properties updated its own target
but did not update the explorer listing or selection. Later actions could use
the old path, especially on providers without live watchers. Successful batch
results now update the listing and selected paths before refreshing. Failed
items retain their original paths. Completed mutations still synchronize when
the dialog closes during the command; refresh errors do not replay the mutation.

The new `--suite hidden` exercises a nonempty-directory deletion control,
unhiding a generated folder, automatic list/selection synchronization without
F5 or reselection, and deletion using the current selection. Independent tree
and byte checks preserve an unrelated sentinel. Its `set_hidden` IPC exception
accepts only owned paths and refuses session-root renames.

Initial candidate: debug `native-test`, base `1a8332a8`, dirty with this correction;
source SHA `630cda8af1b95dc27c89c827e540d95ba232a88438d74508e34e33be7264578f`,
binary SHA `c81bbd10fdb1d61647ff70fc7eb8732c12186a361f3fac62391e504702bd4528`.
This is source-matched candidate evidence, not an installed-build acceptance run.

- `277972d8-5da2-40ee-b058-811055b14b9e`: **BLOCKED**, the driver attempted
  to click the styled checkbox's transparent input. The harness now clicks its
  visible label; no product change was justified by this interception.
- `f5b6dcbc-2ab7-4e17-be72-ab7d3c9e91c6`: local unhide/delete **PASS**;
  mobile unhide/list/selection **PASS**, subsequent folder deletion **FAIL**.
- `5277ccf2-3e55-4551-a03e-8df3079942f9`: local all three parts **PASS**;
  mobile ordinary-folder deletion control and unhide/list/selection **PASS**,
  deletion of the formerly hidden folder **FAIL**. The aggregate remains FAIL.
- `81d73360-d66e-424c-a7dd-76e6132b507f`: fresh local-only run **PASS**,
  all three parts.
- `8216caff-91ba-44b2-9fa0-f6f79dbdde4a`: final local-only run **PASS**,
  all three parts, after tightening the session-root guard to also reject a
  trailing slash. Final source SHA
  `110b86af0eb223a46871471adad9a50b9a85198ab339150d7b592fe9abf92ba7`,
  binary SHA `aeffb8ee285bd7eeaf0f22b2df5a3fda32375b8cb1b4e394289ee6e231b7ee1b`.
  All five runs completed owned candidate/driver teardown.

Frontend tests: 718 passed, including partial Hidden batches, subsequent use of
new paths, closing during an operation and a failed refresh after successful
mutation. Frontend type checks and lint passed. The Rust native-scope regression
rejects root/outside-path Hidden requests before I/O. Blocking Semgrep: zero
findings. Native policy mapping and lint passed after registering the new suite
in the edge tier.

## Separate MTP finding

GVFS MTP 1.60.2-4 / libmtp 1.1.23-1 on this host could not finish removing
some generated folders. Direct GIO returned `could not delete object` or
`Directory not empty`; successful child-delete receipts sometimes left the
child in a fresh provider listing. A small direct-GIO probe also reproduced
the failure in a nonempty hidden folder **without any prior rename**.
Visible-folder controls passed. A unique intermediate name helped a visible
control but did not repair the hidden-folder case. Neither the precise
device/provider cause nor a general workaround has been established.

The maintainer reproduced deletion failure in Nautilus, reconnected USB with
the phone unlocked, and finally removed the test contents with the phone's
own file manager. These observations do not justify attributing this second
failure to Browsey's stale selection or marking MTP deletion accepted.
There is no automatic mutation retry, remount, whole-device scan or successful
deletion claim based solely on a provider acknowledgement.

## Cleanup

Historical local reports, private profiles and recovery data were moved into
a private local archive with content/identity verification. USB, SFTP and
OneDrive generated containers were independently backed up and removed;
their test roots were retained. The local test-only `rclone.conf` was kept.
36 of the remaining 42 generated mobile folders were removed directly. The
maintainer confirmed final phone-side cleanup, including diagnostic fixtures.
The phone was no longer mounted during the final provider check, so this last
step is recorded as manual confirmation rather than an automated empty listing.
Raw reports retain their original PASS/FAIL/BLOCKED outcomes. Private archive
receipts distinguish agent deletion, observed absence and manual device cleanup.

The failed MTP probes remain an open engineering finding; they do not reopen
or rewrite the earlier, differently scoped NT0–NT7 acceptance reports.
