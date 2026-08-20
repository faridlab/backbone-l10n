# backbone-l10n

Country-localization datasets for backbone services — Indonesian (ID) first and
currently the default: `id_sak_chart()` + `id_starter_tax_templates()` are the
datasets to register when a service wants "the" localized chart. Additional
countries join this crate under their own country-prefixed functions rather
than one crate per locale. Data only — no schema, no migrations, no HTTP
surface. Consumed by composing services (e.g. serpa) to install a chart of
accounts and its companion tax templates onto a company.

## Contents

| Path | What |
|---|---|
| `data/id_sak_chart.json` | The Indonesian SAK chart of accounts as a `ChartDataset` (218 accounts / 70 headers / 148 leaves). Committed, diffable. |
| `sources/id_chart_of_accounts.json` | Untouched ERPNext verified source (provenance). Never edited. |
| `tools/convert_erpnext.py` | The deterministic converter: ERPNext tree → `ChartDataset`, with an explicit override table for accounts ERPNext misclassifies. |
| `src/lib.rs` | `id_sak_chart()` (parsed-once dataset) + `id_starter_tax_templates()` (PPN Keluaran/Masukan 11%, PPh 23, PPh 21) + `starter_tax_tag_codes()`. Indonesian is the crate's default and, for now, only country set. |
| `tests/dataset_sanity.rs` | Structural pins (counts, depth, classifications, tax-factor sums, chart-code references). |

## Posture

Datasets are **data**, not database state — no template tables exist anywhere.
The chart-install engine in `backbone-accounting` (v0.6.0+) turns a dataset into
real, manager-editable account rows stamped with `chart_code`/`chart_version`
provenance. Re-installing is the update path.

Starter tax templates are **non-authoritative**: rates are simplified starters
(notably PPh 21 = single TER band placeholder, real PPh 21 is a progressive
gross-up). Corrections ship as a new dataset version, never as in-place edits
under a tenant.

## Regenerating the dataset

```
python3 tools/convert_erpnext.py   # reads sources/, writes data/
cargo test                        # pins must still hold
```

Every classification decision lives in one of three reviewable layers in the
converter: explicit per-number `OVERRIDES`, the ERPNext `account_type` map, or
root+number-prefix defaults. Add an override, not a special case in the walk.
