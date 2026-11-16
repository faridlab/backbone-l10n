# backbone-l10n

Data-only country-localization crate; Indonesian (ID) datasets are the default
and currently the only set. No schema, no migrations, no routes —
if a change needs a migration or an HTTP surface it belongs in a domain module
(`backbone-accounting`, `backbone-tax`), not here.

Rules:

- Never edit `data/id_sak_chart.json` by hand — regenerate it with
  `python3 tools/convert_erpnext.py` and let the sanity pins judge the result.
- Never edit `sources/` — it is provenance, kept byte-identical to upstream.
- Classification fixes go into the converter's `OVERRIDES` table (keyed by
  dots-stripped account number), never into the walk.
- Depends on `backbone-accounting` for the chart-dataset vocabulary ONLY.
  Do not add a dependency on `backbone-tax`: the host maps `TaxTemplateDef`
  onto tax commands, keeping this edge minimal and one-directional.
- Version bumps follow the workspace tag train; the released `Cargo.toml` pins
  `backbone-accounting` by git tag (a sibling `../backbone-accounting` path dep
  is acceptable while developing, and must be flipped back before tagging).

<!-- rust-rules:start (one identical copy in every Rust repo; full text: metaphora docs/handbook/rust-rules.md) -->
## Rust rules (NS01 to NS12)

Every Rust change in this repo follows the twelve Rust rules. The full text, with the reason and
the check for each rule, is the metaphora handbook page
[`docs/handbook/rust-rules.md`](https://github.com/faridlab/metaphora/blob/main/docs/handbook/rust-rules.md).
Cite the code (for example `NS04`) in reviews and commits.

- **NS01 Types carry the meaning:** newtypes and enums for ids, money and states; parse input once, at the boundary.
- **NS02 Never panic on input:** no `unwrap`, `expect`, indexing or unchecked arithmetic on request, file, env, row or message data; return a typed error.
- **NS03 Errors are typed, never swallowed:** `thiserror` in libraries, `anyhow` only in binaries; no `let _ =`, `.ok()` or `unwrap_or_default()` that hides a failure.
- **NS04 Ownership is designed, not appeased:** never add `clone`, `to_vec`, `to_owned`, `Rc`/`Arc`, `RefCell`/`Cell`, `Box`, indices, a split, a lifetime or `unsafe` only to make it compile. `Arc` for state shared across tasks is design, not appeasement.
- **NS05 Unsafe stays in one place:** `#![forbid(unsafe_code)]` by default; any exception lives in one module with `// SAFETY:` on every block.
- **NS06 Pure core, effects at the edge:** domain logic does no database, network, clock or env access; the edge passes those in.
- **NS07 Structure:** one concept per module, private or `pub(crate)` by default, dependencies point inward, generated code and hand code (`*.ext.rs`) kept apart.
- **NS08 Comments and naming:** domain names, comments say why, every public item documented, no session jargon.
- **NS09 Reuse before writing:** search std, current dependencies and the framework first; promote shared code upstream, never copy it.
- **NS10 Dependencies:** each new crate justified, unused default features off, `cargo deny` clean.
- **NS11 Proof, tests, speed:** a fix starts with a test that fails on the old code; a speed claim comes with a measurement.
- **NS12 The CI gate:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -W clippy::pedantic`, `cargo test` and `cargo deny check` all pass; every `#[allow]` says why. Adopted repo by repo: until this repo runs it, reviewers enforce it by hand.
<!-- rust-rules:end -->
