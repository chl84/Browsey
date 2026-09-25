# Thumbnail responsiveness

## Automated checks

```sh
npm --prefix frontend test -- src/features/explorer/thumbnailLoader.test.ts
npm --prefix frontend run test:e2e -- thumbnails.e2e.ts
cargo test commands::thumbnails:: -- --nocapture
```

These checks use synthetic images, temporary cache directories and mocked browser
requests. They do not modify or delete phone files. Coverage includes:

- visible-before-prefetch scheduling, removal of stale queued cards, cancellation
  on navigation and scrolling, bounded video concurrency and overload retries;
- reuse on remount, source-revision invalidation, negative caching and stale
  A/B/A replies;
- a cache with more than 2,000 entries, byte-budget eviction, last-use timestamps
  and protection of temporary output files;
- timeout of a simulated blocked worker while its semaphore permit remains held;
- JPEG generation, cache dimensions, cancellation and terminal decode timeouts;
- metadata validation without opening original file contents, retaining symlink
  rejection.

The synthetic JPEG test prints cold generation and warm cache-lookup timings.
These are diagnostic samples, not a before/after benchmark or an MTP speed claim.

## Desktop / MTP acceptance and measurement

Start the installed app with request timing enabled (no filenames are included in
these completion records):

```sh
RUST_LOG=browsey::commands::thumbnails=debug browsey
```

The app log is normally `~/.local/share/browsey/logs/browsey.log`. Each
`thumbnail request completed` event includes source kind (`local`, `gvfs`,
`cloud`), elapsed milliseconds, cache hit, success and error code.

Measure local storage and an unlocked MTP phone separately. Use the same folder,
window size, sort and visible cards for comparisons. Record time to first image
and to a complete visible screen; separate cold misses from warm cache hits.
Use a new fixture folder for a cold sample rather than deleting the user's cache.
Repeat several times and report median and slow-tail latency, not a single sample.

1. Open a folder containing photos, videos and non-media files. Visible photos
   should load before prefetched cards; non-media files should keep their icons.
2. Scroll several screens down and back. Completed cards should not request new
   thumbnails unless their size/mtime revision changed.
3. Change folders while thumbnails are pending. New cards must not wait for the
   frontend slots of the previous folder, and old replies must not replace them.
4. Return to a previously visited folder. A disk-cache hit still validates source
   metadata and paths, but does not open/decode the original file. This deliberately
   does not trust frontend metadata as filesystem authorization.
5. Toggle video/cloud thumbnails, clear the thumbnail cache through Settings, and
   confirm currently visible cards reload appropriately.
6. Disconnect the phone during loading, then browse a local folder. Remote jobs
   must not consume all local worker capacity. Reconnect and revisit the folder.

## Limits and guarantees

The cache is byte-budgeted (at least 4 KiB charged per entry), not capped at 2,000
files. Cache-file mtime tracks use, coalesced to once per minute. Trimming runs in
the background periodically; the budget is a soft limit during active generation.
New images are published by rename only after generation has completed.

Requests have a total budget of 10 s locally, 12 s on GVfs and 30 s for cloud
sources, including preparation. Raster decoders also retain their shorter codec
budgets. Timeout/cancellation is checked during reads and between stages; JPEG
fallback is reserved for unsupported scaled-decoder pixel formats, not timeouts.
FFmpeg children are killed and reaped on cancellation/deadline.

A blocking OS/GVfs call or CPU-bound third-party renderer cannot always be
forcibly interrupted. The caller still receives a bounded response (25 ms polling
granularity), but the worker retains its permit until it actually finishes.
There are at most eight thumbnail workers, with at most four GVfs/cloud/UNC jobs.
No unbounded replacement decoder pool is created after timeouts.
