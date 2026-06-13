# Vendored dependencies

Third-party Rust crates vendored into the Panes repository for single-repo maintenance.

## claude-code-rust

Embedded as the built-in `claude-code-native` chat engine (`claude_code_rs`).

- Upstream: https://github.com/lorryjovens-hub/claude-code-rust
- Local changes: workspace integration, `blocking` reqwest feature for REPL paths

When syncing upstream, replace `vendor/claude-code-rust/` and re-run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```
