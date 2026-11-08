# Install Loomcoding from source

**What it is**

Loomcoding is built from the Rust source in this repository. This page builds the `loom` binary and checks that it starts.

**What it is for**

- Getting a `loom` binary on your machine.
- Checking the version before you configure anything.

**How**

1. Install the Rust toolchain with rustup.
2. From the repository root, run `cargo build`.
3. Run the binary with `--version`.

## Requirements

| Requirement | Value | Where it is set |
|---|---|---|
| Rust toolchain | `stable`, with `rustfmt` and `clippy` | `rust-toolchain.toml` |
| Binaries built | `loom` (short name) and `loomcoding`, the same program | `crates/loomcoding-cli` |
| Platform checked for this guide | Windows | Secure key storage works only on Windows |

## Example

Windows PowerShell, with an empty folder as profile:

```text
PS> cargo build
PS> New-Item -ItemType Directory -Path <empty-folder>
PS> $env:LOOMCODING_HOME = "<empty-folder>"
PS> target\debug\loom.exe --version
loom 0.1.0
```

Use an empty folder for `LOOMCODING_HOME` while you test. Without it, Loomcoding uses your real profile.

## Settings and defaults

| Name | Default | Requires restart |
|---|---|---|
| `LOOMCODING_HOME` (environment variable) | Not set: the system home folder is the profile | Yes |
| `cargo build` (command) | Not applicable: builds a debug binary in `target/debug/` | Not applicable |

## Result

- `loom 0.1.0` and exit code 0 mean that the binary runs.
- `loom --help` lists `--print`, `--attach` and `--yolo`. Running it creates nothing in the profile folder.
- If `loom` is not found, use the full path `target\debug\loom.exe` from the repository root.

## Next steps

- [What happens on the first run](first-run.md)
