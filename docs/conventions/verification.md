# Verification

- Run `rustfmt --check` on touched Rust files before finalizing.
- Run `cargo check` after UI changes that alter widget structure, state handling, or imports.
- If repo-wide `cargo fmt --check` reports unrelated drift, format only touched files unless the task is explicitly to clean formatting.

