# Verification

- Run `rustfmt --check` on touched Rust files before finalizing.
- Run `cargo check` after UI changes that alter widget structure, state handling, or imports.
- If repo-wide `cargo fmt --check` reports unrelated drift, format only touched files unless the task is explicitly to clean formatting.

For changes that touch application lifecycle, the canvas, media loading, or custom controls,
also run the relevant checks below:

- Normal launch and close must not panic. Keep exactly one Tokio runtime alive while eframe is
  running, and drop it only after leaving its entered context.
- Type shortcut letters and exercise Delete, arrows, Escape, and Ctrl-Z while each text editor,
  autocomplete, and slider owns focus. Canvas/viewer state must not change.
- Page and quick-layout previews must remain paint-only: no transform response may be registered.
- Hidden layers must not paint, hit-test, preview, or export. Locked layers must not move, resize,
  rotate, delete, quick-layout, replace template content, or reorder.
- A pending worker must produce a bounded completion wakeup. Static placeholders must not request
  immediate repaint every pass.
- Export must terminate on ready, failure, timeout, or cancellation; a permanently pending loader
  must not spin forever.
- Large-library tests must measure per-pass data cloning/allocation in addition to widget count.
  Run `cargo test benchmark_cached_large_library_query_snapshot -- --ignored --nocapture` for the
  checked-in 25,000-photo query-snapshot benchmark.
- Layout tests should cover changing job labels, fonts/style/zoom, width, and the first visible pass.
- Run `cargo test --features wgpu,snapshot` when the machine has the required rendering backend;
  default tests do not execute every snapshot-gated test.
- When enabling `debug_dependency_locks` for the whole suite, use
  `cargo test --all-features -- --test-threads=1`: singleton integration tests share process-global
  dependencies and are not isolated from parallel mutation.
