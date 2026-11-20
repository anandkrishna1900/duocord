# Contributing to PrivCord

Thanks for helping. PrivCord is a private, end-to-end encrypted, peer-to-peer messenger (Rust engine, Android and Windows apps).

- **Security problems:** report privately (see [SECURITY.md](SECURITY.md)), not in a public issue.
- **Bugs and ideas:** open an issue with what you expected, what happened, your platform and version, and steps to reproduce. Do not paste real conversations.
- **Code:** keep changes small and focused. Before opening a pull request run
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`,
  `cargo test --workspace --all-features --locked` and, for the apps, `npm run typecheck && npm test`
  (see [For developers](README.md#for-developers)). Protocol or crypto changes need tests, and new parsers need a fuzz target.
- **License:** PrivCord is source-available under the PC-NC license (see [LICENSE](LICENSE)); by contributing you agree your
  contribution may be distributed under it.
