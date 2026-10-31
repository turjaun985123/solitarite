# Repository instructions

## Build and verification

- Do not run Cargo locally. This includes `cargo build`, `cargo test`, `cargo check`, `cargo clippy`, `cargo fmt`, `cargo doc`, and other Cargo commands.
- Do not perform local builds or run build/test workflows through another tool as a substitute for Cargo.
- Build and test verification must run only through the repository's GitHub Actions CI workflows.
- You may inspect and edit source files and workflow configuration locally, but report verification as pending until GitHub Actions CI has run.
