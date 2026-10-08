# zenoh-2825-repro

Reproduces [zenoh#2825](https://github.com/eclipse-zenoh/zenoh/issues/2825) using zenoh only, and shows that the fix on [`tomkimsour/zenoh` `fix/2825-reclaim-expr-ids`](https://github.com/tomkimsour/zenoh/tree/fix/2825-reclaim-expr-ids) removes it.

zenoh gives every key prefix it declares on a connection a 16-bit id, and before the fix those ids were never released. A peer that keeps declaring and undeclaring liveliness tokens on new keys exhausts them after 65,535 declarations; the next declaration then spins forever while holding the routing lock, and nothing is routed any more.

## Layout

| Path | Content |
| --- | --- |
| `zenoh-before/` | submodule: `eclipse-zenoh/zenoh` at `74051d0cc`, upstream `main` without the fix |
| `zenoh-after/` | submodule: `tomkimsour/zenoh` at `296ca98a3`, the same commit plus the fix |
| `scenario/main.rs` | the scenario, written once against the zenoh API |
| `scenario-before/`, `scenario-after/` | build `scenario/main.rs` against each submodule |
| `src/` | the runner: builds both scenarios, runs them, compares the results |

The scenario opens a router and a peer session in one process, connected over TCP loopback. The peer declares then undeclares one liveliness token per new key. It reports a freeze when no token is declared for 10 s, then checks that a `put` from the peer still reaches a subscriber on the router, retrying for up to 60 s.

## Requirements

- Rust toolchain 1.97.1 (pinned by `rust-toolchain.toml`, installed by rustup on first use)
- git, network access to GitHub for the submodules and crates.io

## Usage

```bash
git clone --recurse-submodules <this repository> zenoh-2825-repro
cd zenoh-2825-repro
cargo run --release
```

On an existing clone, fetch the submodules first with `git submodule update --init`.

`cargo run --release -- <count>` sets the number of tokens (default 100000). Counts of 65535 or less do not reach the bug.

The first run compiles zenoh twice. The run on `zenoh-before` then takes several minutes, because each declaration gets slower until the freeze; the run on `zenoh-after` takes about a second.

The command exits with 0 when the bug shows on `zenoh-before` and not on `zenoh-after`, and with 1 otherwise.

Each scenario can also be run on its own:

```bash
cargo run --release --manifest-path scenario-before/Cargo.toml -- 100000 17447
cargo run --release --manifest-path scenario-after/Cargo.toml -- 100000 17448
```

The runner's result parsing and verdict are covered by `cargo test`.
