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
