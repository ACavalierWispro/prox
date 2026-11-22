# libs/

Rust dependencies are managed by Cargo and listed in `Cargo.toml` under `[dependencies]`
(rust-analyzer, VS Code, Zed and Neovim all read that file; nothing else to configure).

Use this folder for local crates: put a crate here and reference it with
`my_crate = { path = "libs/my_crate" }`.
