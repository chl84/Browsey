# Stable UnRAR source refresh

Bindings originate from `unrar_sys` 0.5.8 (MIT OR Apache-2.0), archive SHA-256
`8b77675b883cfbe6bf41e6b7a5cd6008e0a83ba497de3d96e41a064bbeead765`.
The native source is the complete, unmodified stable UnRAR 7.23 distribution:
https://www.rarlab.com/rar/unrarsrc-7.2.7.tar.gz
Archive SHA-256:
`01d903a7dcf413cb2925696d7796e48e38d471f79bfe7ef3ad2aebf6c12dbefd`.
The archive's filename is a source-package revision, not the native version;
`vendor/unrar/version.hpp` records 7.23 with no beta suffix.

Browsey changes the wrapper manifest/build file to use this source, adds the
new `resource`/`largepage` translation units, preserves the RARDLL build, and
matches upstream's packed structures and newly named formerly reserved fields.
The small MIT-licensed ABI probe checks native sizes, critical offsets, DLL
interface version and actual compiled native version in application regressions.
Passwords, cancellation, safety limits and output files remain under Browsey's
existing streaming adapter. Native code does not choose extraction destinations.

The adapter serializes native handles, including open and close, because UnRAR's
DLL error handler is process-global. Waiting for a handle remains cancellable.
The 7.23 missing-header-password result is mapped using the password callback's
actual request state, so a missing archive or volume never prompts for a password.

UnRAR source licensing: `vendor/unrar/license.txt`, also packaged as
`resources/unrar-LICENSE.txt`. Native-source and patched-wrapper tree hashes
are recorded in `vendor/provenance.json` and checked in dependency CI.
Keep ABI probes and archive/password/volume regressions when refreshing again.
