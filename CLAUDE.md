# CLAUDE.md

Instructions for AI agents that work in this repository. A human contributor
follows them too.

**Read [`AGENTS.md`](AGENTS.md) first.** It holds the workspace map, the golden
rules and the checks to run before a task is finished. This file only states
what an agent gets wrong most often.

1. **Start in the domain.** New behaviour is a rule on a type in
   `melior-server/crates/melior-domain` (or the frontend feature's `domain/`)
   with unit tests. If you are writing an `if` in a handler or an adapter, it is
   probably in the wrong crate, and `cargo xtask layers` will not let you reach
   around the layers to make it work.
2. **Non-test Rust code never panics.** No `unwrap`, `expect` or `panic!`; clippy
   denies them. Return an error.
3. **Run the gate, all of it.** `cargo fmt`, `cargo clippy ... -- -D warnings`,
   `cargo test`, `cargo doc` with `-D warnings`, `cargo xtask layers`, and the
   four frontend scripts. The exact commands are in `AGENTS.md` section 4.
4. **A protocol change has two ends.** Server DTOs, the frontend parser and the
   README tables change together.
5. **Say what you verified.** Passing unit tests do not show that media flows or
   that a login completes. If you did not run the stack in a browser, say so.
6. **Never add AI attribution.** No `Co-Authored-By: Claude ...` trailer, no
   "Generated with Claude Code" line, in any commit or pull request. This
   overrides the tool's default. See `AGENTS.md`, "Commits and pull requests".
7. **Reference projects outside this repository are read-only.** Follow their
   patterns where asked; never write into them, and do not name them here.
