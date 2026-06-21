# Vendor Directory

This directory is reserved for third-party reference trees and vendored assets
that are not part of the main `panes-agent` native runtime.

`vendor/claude-code-rust/` has been removed. `claude-code-native` is now a
legacy engine id alias routed to `claurst-native`; the active native runtime is
implemented in `crates/panes-agent`.

`vendor/claurst/`, when present as a local checkout or submodule, is a GPL-3.0
third-party behavior reference only. It is not a Panes product component, is not
compiled by the workspace, and is explicitly not compiled or linked into Panes
binaries. Release source and binary packages may exclude it or must keep this
third-party status clear.
