//! Run the application's GLib regression without optimizing the whole application.
//!
//! The workspace shares Browsey's lockfile, GIO pin and vendored GLib patch.

#[cfg(all(test, not(target_os = "windows")))]
#[path = "../../../src/gtk_regression_tests.rs"]
mod gtk_regression_tests;
