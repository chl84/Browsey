# RAR test fixtures

- `version.rar`, `unicode-entry.rar`, and `crypted.rar` are copied from the
  [`unrar` crate test data](https://github.com/muja/unrar.rs/tree/0.5.8/data),
  which is dual-licensed MIT or Apache-2.0.
- `rar5-compressed.rar` and `test_read_format_rar5_multiarchive.part*.rar`
  are decoded from the [libarchive test corpus](https://github.com/libarchive/libarchive/tree/master/libarchive/test)
  and are covered by libarchive's BSD-2-Clause license.

The RAR5 fixtures exercise compressed decoding and automatic continuation over
eight archive volumes. They are test data only and are not embedded in Browsey
release artifacts.
