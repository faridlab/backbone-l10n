//! Dataset sanity — pins the shipped Indonesian dataset so edits are conscious.
//! Pure: parses the compiled-in data and asserts structural invariants; no DB.

use backbone_accounting::domain::chart_dataset::validate_dataset;
use backbone_accounting::domain::entity::AccountSubtype;
use backbone_l10n::{
    id_efaktur_csv_column_set, id_sak_chart, id_starter_tax_templates, starter_tax_tag_codes,
    EFakturCsvRecord,
};
use rust_decimal::Decimal;
use std::collections::HashSet;

#[test]
fn dataset_passes_engine_validation() {
    assert!(validate_dataset(&id_sak_chart()).is_ok());
}

#[test]
fn count_pins_218_accounts_148_leaves_70_headers() {
    let ds = id_sak_chart();
    assert_eq!(ds.code, "ID_SAK");
    assert_eq!(ds.accounts.len(), 218);

    // Headers = accounts that are somebody's parent (the engine derives
    // is_header exactly this way); leaves = the rest.
    let parent_codes: HashSet<&str> = ds
        .accounts
        .iter()
        .filter_map(|a| a.parent_code.as_deref())
        .collect();
    let leaves = ds
        .accounts
        .iter()
        .filter(|a| !parent_codes.contains(a.code.as_str()))
        .count();
    assert_eq!(parent_codes.len(), 70, "header accounts");
    assert_eq!(leaves, 148, "leaf accounts");
    assert_eq!(ds.accounts.len(), 70 + 148);
}

#[test]
fn tree_depth_at_most_5() {
    let ds = id_sak_chart();
    let mut depth: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    let mut max = 0u32;
    for a in &ds.accounts {
        let d = match a.parent_code.as_deref() {
            None => 0,
            Some(p) => depth[p] + 1,
        };
        depth.insert(a.code.as_str(), d);
        max = max.max(d);
    }
    assert_eq!(max, 5, "SAK tree deepest level");
}

#[test]
fn classification_pins() {
    let ds = id_sak_chart();
    let by_code = |code: &str| {
        ds.accounts
            .iter()
            .find(|a| a.code == code)
            .unwrap_or_else(|| panic!("code {code} in dataset"))
    };

    // Tax accounts: side follows the parent's side of the chart.
    let ppn_out = by_code("2142000"); // PPN Keluaran
    assert_eq!(ppn_out.account_type.to_string(), "liability");
    assert_eq!(ppn_out.account_subtype, AccountSubtype::Tax);
    let ppn_in = by_code("1152001"); // PPN Masukan
    assert_eq!(ppn_in.account_subtype, AccountSubtype::Tax);
    assert_eq!(ppn_in.account_type.to_string(), "asset");

    // Hutang Pajak is a tax clearing account, never a party payable.
    let hutang_pajak = by_code("2141000");
    assert_eq!(hutang_pajak.account_subtype, AccountSubtype::Tax);
    assert!(!hutang_pajak.is_reconcilable);

    // Bank/cash are reconcilable; an unnumbered-advance is not a bank.
    assert!(by_code("1121000").is_reconcilable); // Bank Rupiah
    assert!(!by_code("1142001").is_reconcilable); // Uang Muka Pembelian

    // Contra balances.
    use backbone_accounting::domain::entity::NormalBalance;
    assert_eq!(by_code("4120000").normal_balance, NormalBalance::Debit); // Retur Penjualan
    assert_eq!(by_code("3120000").normal_balance, NormalBalance::Debit); // Prive
                                                                         // The accumulated-depreciation header matches its credit-balance children.
    assert_eq!(
        by_code("1212000").account_subtype,
        AccountSubtype::AccumulatedDepreciation
    );
    assert_eq!(by_code("1212001").normal_balance, NormalBalance::Credit); // Akumulasi Penyusutan

    // COGS and other-income routing.
    assert_eq!(by_code("4210000").account_type.to_string(), "cogs");
    assert_eq!(by_code("4410000").account_type.to_string(), "other_income");
    assert_eq!(by_code("5510009").account_type.to_string(), "other_expense");
}

#[test]
fn reconcilable_only_on_party_bank_cash_subtypes() {
    let ok = [
        AccountSubtype::Bank,
        AccountSubtype::Cash,
        AccountSubtype::AccountsReceivable,
        AccountSubtype::AccountsPayable,
    ];
    for a in &id_sak_chart().accounts {
        if a.is_reconcilable {
            assert!(
                ok.contains(&a.account_subtype),
                "{} ({}) is reconcilable but not a party/bank/cash subtype",
                a.number,
                a.name
            );
        }
    }
}

#[test]
fn required_operational_subtypes_present() {
    let ds = id_sak_chart();
    let count = |sub: AccountSubtype| {
        ds.accounts
            .iter()
            .filter(|a| a.account_subtype == sub)
            .count()
    };
    assert_eq!(count(AccountSubtype::Bank), 3);
    assert_eq!(count(AccountSubtype::Cash), 4);
    assert_eq!(count(AccountSubtype::AccountsReceivable), 2);
    assert_eq!(count(AccountSubtype::AccountsPayable), 9);
}

#[test]
fn tax_defs_reference_existing_chart_codes() {
    let ds = id_sak_chart();
    let codes: HashSet<&str> = ds.accounts.iter().map(|a| a.code.as_str()).collect();
    for t in id_starter_tax_templates() {
        for r in &t.rows {
            if let Some(c) = &r.account_code {
                assert!(
                    codes.contains(c.as_str()),
                    "{} row references missing {}",
                    t.code,
                    c
                );
            }
        }
        for f in &t.families {
            for s in &f.tax_splits {
                if let Some(c) = &s.account_code {
                    assert!(
                        codes.contains(c.as_str()),
                        "{} family references missing {}",
                        t.code,
                        c
                    );
                }
            }
        }
    }
}

#[test]
fn tax_split_factors_sum_100_per_family() {
    for t in id_starter_tax_templates() {
        for f in &t.families {
            let sum: Decimal = f.tax_splits.iter().map(|s| s.factor_percent).sum();
            assert_eq!(
                sum,
                Decimal::ONE_HUNDRED,
                "{} / {}",
                t.code,
                f.document_type
            );
        }
    }
}

#[test]
fn tax_rows_within_rate_range_and_tag_codes_resolve() {
    let tags: HashSet<String> = starter_tax_tag_codes().into_iter().collect();
    assert_eq!(tags.len(), 4, "PPN-KELUARAN, PPN-MASUKAN, PPH-23, PPH-21");

    let templates = id_starter_tax_templates();
    assert_eq!(templates.len(), 4);
    for t in &templates {
        assert!(
            matches!(t.template_type.as_str(), "sales" | "purchase"),
            "{} template_type",
            t.code
        );
        for r in &t.rows {
            assert!(
                r.rate > Decimal::ZERO && r.rate <= Decimal::ONE_HUNDRED,
                "{} rate",
                t.code
            );
            if r.effective_to.is_some() {
                assert!(
                    r.effective_to.unwrap() >= r.effective_from,
                    "{} window",
                    t.code
                );
            }
        }
        for f in &t.families {
            for c in &f.base_tag_codes {
                assert!(tags.contains(c), "{} base tag {}", t.code, c);
            }
            for s in &f.tax_splits {
                for c in &s.tag_codes {
                    assert!(tags.contains(c), "{} split tag {}", t.code, c);
                }
            }
        }
    }
}

// ── e-Faktur CSV column set ───────────────────────────────────────────────────

#[test]
fn efaktur_csv_column_set_shape_pins() {
    let set = id_efaktur_csv_column_set();
    assert!(!set.version.is_empty(), "the set carries a version");
    assert_eq!(set.columns.len(), 25, "16 FK header + 9 OF detail columns");

    let fk = set.columns_for(EFakturCsvRecord::Fk);
    let of = set.columns_for(EFakturCsvRecord::Of);
    assert_eq!(fk.len(), 16, "FK header record columns");
    assert_eq!(of.len(), 9, "OF detail record columns");

    // Emission order is contiguous 1..=n within each record.
    let fk_orders: Vec<i32> = fk.iter().map(|c| c.order).collect();
    assert_eq!(
        fk_orders,
        (1..=16).collect::<Vec<i32>>(),
        "FK order contiguous"
    );
    let of_orders: Vec<i32> = of.iter().map(|c| c.order).collect();
    assert_eq!(
        of_orders,
        (1..=9).collect::<Vec<i32>>(),
        "OF order contiguous"
    );

    // Keys are unique and derive from record tag + label; labels/formats are filled in.
    let keys: HashSet<&str> = set.columns.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(keys.len(), set.columns.len(), "column keys unique");
    for c in &set.columns {
        let prefix = c.record.tag();
        assert_eq!(c.key, format!("{}_{}", prefix, c.label), "{} key", c.key);
        assert!(!c.label.is_empty(), "{} label", c.key);
        assert!(!c.format.is_empty(), "{} format", c.key);
        assert!(!c.source_note.is_empty(), "{} source note", c.key);
        assert!(
            c.source_note.contains("PER-24/PJ/2019"),
            "{} cites the regulation",
            c.key
        );
        if let Some(to) = c.effective_to {
            assert!(to >= c.effective_from, "{} window", c.key);
        }
    }

    // The whole set is pending review — the reviewer flip is a new version, never
    // an in-place edit that this pin would silently bless.
    assert!(
        set.columns.iter().all(|c| c.reviewer_status == "pending"),
        "every column ships reviewer_status=pending"
    );

    // Effective-dating: the set's window covers its columns' windows.
    for c in &set.columns {
        assert!(
            c.effective_from >= set.effective_from,
            "{} inside the set window",
            c.key
        );
    }
}

#[test]
fn efaktur_csv_core_columns_carry_their_masks() {
    let set = id_efaktur_csv_column_set();
    let fk = |label: &str| {
        set.columns
            .iter()
            .find(|c| c.record == EFakturCsvRecord::Fk && c.label == label)
            .unwrap_or_else(|| panic!("FK_{label} in the set"))
    };

    // The number column emits the 19-char DJP mask; the date is dd/mm/yyyy.
    assert_eq!(fk("NOMOR_FAKTUR").format, "mask:010.NNN-NN.YYYYYYYY");
    assert_eq!(fk("TANGGAL_FAKTUR").format, "date:dd/mm/yyyy");

    // The FK/OF tags the exporter writes as each record's first field.
    assert_eq!(EFakturCsvRecord::Fk.tag(), "FK");
    assert_eq!(EFakturCsvRecord::Of.tag(), "OF");
}
