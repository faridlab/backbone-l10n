//! Dataset sanity — pins the shipped Indonesian dataset so edits are conscious.
//! Pure: parses the compiled-in data and asserts structural invariants; no DB.

use backbone_accounting::domain::chart_dataset::validate_dataset;
use backbone_accounting::domain::entity::AccountSubtype;
use backbone_l10n::{id_sak_chart, id_starter_tax_templates, starter_tax_tag_codes};
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
    assert_eq!(by_code("1212000").account_subtype, AccountSubtype::AccumulatedDepreciation);
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
                assert!(codes.contains(c.as_str()), "{} row references missing {}", t.code, c);
            }
        }
        for f in &t.families {
            for s in &f.tax_splits {
                if let Some(c) = &s.account_code {
                    assert!(codes.contains(c.as_str()), "{} family references missing {}", t.code, c);
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
            assert_eq!(sum, Decimal::ONE_HUNDRED, "{} / {}", t.code, f.document_type);
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
            assert!(r.rate > Decimal::ZERO && r.rate <= Decimal::ONE_HUNDRED, "{} rate", t.code);
            if r.effective_to.is_some() {
                assert!(r.effective_to.unwrap() >= r.effective_from, "{} window", t.code);
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
