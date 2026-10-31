# Condite

## Rust developer checks

The Rust service lives in `core/`. Install the stable Rust toolchain with the
`rustfmt` and `clippy` components, then run the repository checks from the
project root:

```sh
./scripts/check-rust.sh
```

The script checks formatting with `cargo fmt --check` and runs Clippy for all
targets and features, followed by the test suite. Clippy warnings fail the check so new warnings are
visible in local development and CI. The same script runs in the Rust checks
GitHub Actions workflow.
