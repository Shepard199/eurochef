# EuroChef legacy build rules

- Use `CARGO.cmd` for Cargo commands and `RUN_GUI.cmd` for the GUI. Do not invoke a bare system or CodexPro `cargo.exe` against this workspace.
- The canonical crate registry is `.cargo-local`; the canonical compiler is the pinned toolchain from `rust-toolchain.toml`; build artifacts stay in `target`.
- Do not override `CARGO_HOME`, `RUSTUP_HOME`, `CARGO_TARGET_DIR`, `RUSTC`, Rust wrappers, or Rust flags in scripts or agent commands.
- Keep dependency-resolving commands locked. `CARGO.cmd` adds `--locked` to build/check/test/clippy/run/bench automatically.
- Never run `cargo clean` or delete `target`, `.cargo-local/registry`, or Cargo health/context markers unless the user explicitly requests a clean rebuild.
- A local source edit may rebuild the affected EuroChef workspace crate. It must not rebuild third-party registry crates because their source path changed.
