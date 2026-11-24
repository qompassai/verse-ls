# verse-ls

verse-ls is a language server for Epic's Verse language (UEFN today,
UE6 gameplay programming next), written in Rust. It exists because
Verse's only other language server is Epic's closed, VS Code-only
binary; verse-ls is built for the estate's Neovim-first workflow in
Diver. It is developed rung by rung — document lifecycle first, then
parse diagnostics from a hand-written parser, then digest-backed API
navigation, then a workspace index — and every rung ships only behind
its own gate. The target (Fortnite version, digest-set hash) is
recorded in [TARGET.md](TARGET.md).

**Honesty rule:** verse-ls is advisory tooling. It never claims, in
any diagnostic, message, or document, that Verse code compiles.
**Build Verse Code in UEFN remains the only compile gate** — this
server checks syntax, makes the API surface navigable, and indexes
your own code; compilation is UEFN's word, not ours.

Status: Rung 0 (skeleton) — lifecycle and document sync only, no
Verse intelligence yet.

Build and test on the pinned toolchain (`rust-toolchain.toml`):

```sh
cargo build
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Licence: Apache-2.0.
