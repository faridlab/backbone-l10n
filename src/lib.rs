//! Indonesian localization datasets for backbone services — the data half of the
//! chart-install posture: datasets are DATA, there are no template tables
//! anywhere. Installing writes ordinary, manager-editable account rows.
//!
//! Ships:
//! - the Indonesian SAK chart of accounts (`ID_SAK`, 218 accounts), converted
//!   from ERPNext's verified Indonesian chart. Provenance: `sources/` holds the
//!   untouched upstream file; `tools/convert_erpnext.py` is the deterministic,
//!   reviewable conversion; `data/id_sak_chart.json` is its output, committed
//!   so the dataset is diffable in code review.
//! - a starter set of Indonesian tax templates (PPN Keluaran/Masukan 11%,
//!   PPh 23, PPh 21) expressed in this crate's own [`TaxTemplateDef`] shape;
//!   the composing service maps them onto backbone-tax write commands at
//!   install time. Rates and classifications are STARTER data, non-authoritative:
//!   corrections ship as a new chart/template version, never as in-place edits
//!   under a tenant.
//!
//! Only backbone-accounting's enum vocabulary is imported. The tax module is
//! deliberately not a dependency — the edge stays minimal and one-directional.

use backbone_accounting::domain::chart_dataset::ChartDataset;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::sync::{Arc, OnceLock};

const ID_SAK_JSON: &str = include_str!("../data/id_sak_chart.json");

/// The Indonesian SAK chart of accounts, parsed once. A malformed shipped
/// dataset is a programmer error and panics — the data is compiled in.
pub fn id_sak_chart() -> Arc<ChartDataset> {
    static CHART: OnceLock<Arc<ChartDataset>> = OnceLock::new();
    CHART
        .get_or_init(|| {
            let ds: ChartDataset = serde_json::from_str(ID_SAK_JSON)
                .expect("shipped ID_SAK dataset must parse against ChartDataset");
            backbone_accounting::domain::chart_dataset::validate_dataset(&ds)
                .expect("shipped ID_SAK dataset must satisfy dataset invariants");
            Arc::new(ds)
        })
        .clone()
}

/// One Indonesian tax template, in dataset form. The host's install verb maps
/// this onto backbone-tax commands: `NewTemplate` from the header, `NewTemplateRow`
/// per row, `create_tag` + `ReplaceRepartitionFamily` per family. Company and
/// account ids are resolved by the host — here everything is keyed by chart
/// account `code` (dots-stripped number) so definitions stay tenant-neutral.
#[derive(Debug, Clone)]
pub struct TaxTemplateDef {
    /// Company-unique template code (e.g. `PPN-KELUARAN-11`).
    pub code: String,
    pub name: String,
    /// `sales` | `purchase`.
    pub template_type: String,
    /// Tax included in the quoted price?
    pub is_inclusive: bool,
    /// Cash-basis posture override; `None` = the company default.
    pub tax_exigibility: Option<String>,
    pub rows: Vec<TaxRateRowDef>,
    pub families: Vec<RepartitionFamilyDef>,
}

/// One rate row of a template (backbone-tax `NewTemplateRow`, dataset-shaped).
#[derive(Debug, Clone)]
pub struct TaxRateRowDef {
    /// `on_net_total` | `on_previous_row_total` | `actual`; `None` = module default.
    pub charge_type: Option<String>,
    /// Percent: `11` = 11%.
    pub rate: Decimal,
    /// Chart account code the tax posts to, if any.
    pub account_code: Option<String>,
    pub is_withholding: bool,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    pub sort_order: i32,
}

/// One document-type repartition family (`invoice` | `refund`), dataset-shaped.
#[derive(Debug, Clone)]
pub struct RepartitionFamilyDef {
    pub document_type: String,
    /// Tag codes for the family's base line.
    pub base_tag_codes: Vec<String>,
    pub base_description: Option<String>,
    pub tax_splits: Vec<TaxSplitDef>,
}

/// One tax split of a family; factors must sum to 100 per family.
#[derive(Debug, Clone)]
pub struct TaxSplitDef {
    pub factor_percent: Decimal,
    pub account_code: Option<String>,
    pub tag_codes: Vec<String>,
    pub sort_order: i32,
    pub description: Option<String>,
}

/// All tag codes referenced anywhere in the starter set — the host creates each
/// (find-by-code first, so re-installs stay idempotent) before wiring families.
pub fn starter_tax_tag_codes() -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for t in id_starter_tax_templates() {
        for f in &t.families {
            for c in &f.base_tag_codes {
                if !codes.iter().any(|x| x == c) {
                    codes.push(c.clone());
                }
            }
            for s in &f.tax_splits {
                for c in &s.tag_codes {
                    if !codes.iter().any(|x| x == c) {
                        codes.push(c.clone());
                    }
                }
            }
        }
    }
    codes
}

/// Starter Indonesian tax templates. Non-authoritative: PPh 21 in particular is
/// a single-band simplification of the TER progressive schedule — the payroll
/// posture replaces it with the full table.
pub fn id_starter_tax_templates() -> Vec<TaxTemplateDef> {
    let ppn_from = NaiveDate::from_ymd_opt(2022, 4, 1).expect("valid date"); // PPN 11% since April 2022
    let pph_from = NaiveDate::from_ymd_opt(2022, 1, 1).expect("valid date");
    let pph21_from = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"); // TER era

    let family = |document_type: &str, account_code: &str, tag: &str| RepartitionFamilyDef {
        document_type: document_type.to_string(),
        base_tag_codes: vec![tag.to_string()],
        base_description: Some(format!("{tag} base")),
        tax_splits: vec![TaxSplitDef {
            factor_percent: Decimal::ONE_HUNDRED,
            account_code: Some(account_code.to_string()),
            tag_codes: vec![tag.to_string()],
            sort_order: 1,
            description: None,
        }],
    };

    vec![
        // Output VAT collected on sales → PPN Keluaran (liability).
        TaxTemplateDef {
            code: "PPN-KELUARAN-11".into(),
            name: "PPN Keluaran 11%".into(),
            template_type: "sales".into(),
            is_inclusive: false,
            tax_exigibility: None,
            rows: vec![TaxRateRowDef {
                charge_type: Some("on_net_total".into()),
                rate: Decimal::from(11),
                account_code: Some("2142000".into()),
                is_withholding: false,
                effective_from: ppn_from,
                effective_to: None,
                sort_order: 1,
            }],
            families: vec![
                family("invoice", "2142000", "PPN-KELUARAN"),
                family("refund", "2142000", "PPN-KELUARAN"),
            ],
        },
        // Input VAT paid on purchases → PPN Masukan (asset, creditable).
        TaxTemplateDef {
            code: "PPN-MASUKAN-11".into(),
            name: "PPN Masukan 11%".into(),
            template_type: "purchase".into(),
            is_inclusive: false,
            tax_exigibility: None,
            rows: vec![TaxRateRowDef {
                charge_type: Some("on_net_total".into()),
                rate: Decimal::from(11),
                account_code: Some("1152001".into()),
                is_withholding: false,
                effective_from: ppn_from,
                effective_to: None,
                sort_order: 1,
            }],
            families: vec![
                family("invoice", "1152001", "PPN-MASUKAN"),
                family("refund", "1152001", "PPN-MASUKAN"),
            ],
        },
        // Article 23 withholding on services (2%). Withheld as the tax agent on
        // vendor payments, so the amount is owed to the tax office — Hutang Pajak
        // (liability), not the company's own prepaid-tax asset.
        TaxTemplateDef {
            code: "PPH-23-2".into(),
            name: "PPh Pasal 23 2%".into(),
            template_type: "purchase".into(),
            is_inclusive: false,
            tax_exigibility: None,
            rows: vec![TaxRateRowDef {
                charge_type: Some("on_net_total".into()),
                rate: Decimal::from(2),
                account_code: Some("2141000".into()),
                is_withholding: true,
                effective_from: pph_from,
                effective_to: None,
                sort_order: 1,
            }],
            families: vec![
                family("invoice", "2141000", "PPH-23"),
                family("refund", "2141000", "PPH-23"),
            ],
        },
        // Article 21 withholding on payroll — single-band TER placeholder;
        // the payroll posture replaces this with the progressive table.
        TaxTemplateDef {
            code: "PPH-21-3".into(),
            name: "PPh Pasal 21 3% (TER placeholder)".into(),
            template_type: "purchase".into(),
            is_inclusive: false,
            tax_exigibility: None,
            rows: vec![TaxRateRowDef {
                charge_type: Some("on_net_total".into()),
                rate: Decimal::from(3),
                account_code: Some("2141000".into()),
                is_withholding: true,
                effective_from: pph21_from,
                effective_to: None,
                sort_order: 1,
            }],
            families: vec![
                family("invoice", "2141000", "PPH-21"),
                family("refund", "2141000", "PPH-21"),
            ],
        },
    ]
}
