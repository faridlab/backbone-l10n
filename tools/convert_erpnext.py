#!/usr/bin/env python3
"""Convert the ERPNext Indonesian chart of accounts into a backbone ChartDataset.

Source: ERPNext's verified `id_chart_of_accounts.json` (218 nodes / 70 groups /
148 leaves). The source nests accounts as dicts keyed by (Indonesian) name, with
`account_number`, `account_type`, and `is_group` carried as sibling scalar keys.

Output: `data/id_sak_chart.json` — the parents-first account list the
backbone-accounting chart install engine consumes, with every account classified
into backbone's (account_type, account_subtype, normal_balance) vocabulary.

The mapping is deterministic and reviewable in three layers, applied in order:

1. Explicit per-number overrides (`OVERRIDES`, keyed by code = number with dots
   stripped) for accounts whose ERPNext classification is wrong or unhelpful for
   double-entry use (e.g. 2141.000 is typed `Payable` but is a tax clearing
   account; 4120/4130 are contra-revenue and carry a debit balance).
2. ERPNext `account_type` mapping, where present and unambiguous for the root
   the account sits under.
3. Root + number-prefix defaults for untyped accounts (the majority: tree
   position implies classification in the SAK numbering plan).

Run:  python3 tools/convert_erpnext.py
"""

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "sources" / "id_chart_of_accounts.json"
TARGET = ROOT / "data" / "id_sak_chart.json"

DATASET_CODE = "ID_SAK"
DATASET_VERSION = "2026.1"
DATASET_NAME = "Bagan Akun Standar (Indonesia)"

# (account_type, account_subtype, normal_balance, is_reconcilable)
OVERRIDES = {
    # 2141.000 Hutang Pajak — ERPNext types it Payable; it is the PPh withholding
    # clearing liability. Party semantics on a tax account are wrong.
    "2141000": ("liability", "tax", "credit", False),
    # 2132.001 freight accrual — ERPNext's "Expenses Included In Valuation" is a
    # plain current liability, not a party payable.
    "2132001": ("liability", "current_liability", "credit", False),
    # 2121.001 Dp Penjualan — ERPNext types it Bank, but a customer deposit is a
    # current liability, not a bank account.
    "2121001": ("liability", "current_liability", "credit", False),
    # 1121/1122 are marked is_group in ERPNext but carry no children — they are
    # the bank accounts companies actually post to.
    "1121000": ("asset", "bank", "debit", True),
    "1122000": ("asset", "bank", "debit", True),
    # 1142.001 Uang Muka Pembelian — ERPNext types it Bank, but a supplier
    # advance is a current asset, not a bank account to reconcile.
    "1142001": ("asset", "current_asset", "debit", False),
    # Contra-revenue carries a debit balance.
    "4120000": ("revenue", "operating_revenue", "debit", False),  # Retur Penjualan
    "4130000": ("revenue", "operating_revenue", "debit", False),  # Potongan Penjualan
    # Prive (owner drawings) — contra-equity, debit balance.
    "3120000": ("equity", "paid_in_capital", "debit", False),
    # 1212.000 Akumulasi Penyusutan header — ERPNext leaves the group untyped,
    # so the prefix default made it a current-asset/debit while its own child
    # (1212001) is accumulated-depreciation/credit. The header must match.
    "1212000": ("asset", "accumulated_depreciation", "credit", False),
}

# ERPNext account_type → backbone classification, per root the account sits under.
# Untyped entries fall through to the prefix defaults below.
ERP_TYPE_MAP = {
    "Aktiva": {
        "Bank": ("asset", "bank", "debit", True),
        "Cash": ("asset", "cash", "debit", True),
        "Receivable": ("asset", "accounts_receivable", "debit", True),
        "Tax": ("asset", "tax", "debit", False),
        "Fixed Asset": ("asset", "fixed_asset", "debit", False),
        "Accumulated Depreciation": ("asset", "accumulated_depreciation", "credit", False),
        "Stock": ("asset", "inventory", "debit", False),
        "Temporary": ("asset", "current_asset", "debit", False),
    },
    "Passiva": {
        "Payable": ("liability", "accounts_payable", "credit", True),
        "Tax": ("liability", "tax", "credit", False),
        "Stock Received But Not Billed": ("liability", "current_liability", "credit", False),
        "Expenses Included In Valuation": ("liability", "current_liability", "credit", False),
    },
    "Penjualan": {
        "Income Account": ("revenue", "operating_revenue", "credit", False),
        "Cost of Goods Sold": ("cogs", "direct_cost", "debit", False),
    },
    "Beban": {
        "Round Off": ("other_expense", "operating_expense", "debit", False),
        "Stock Adjustment": ("expense", "operating_expense", "debit", False),
        "Depreciation": ("expense", "operating_expense", "debit", False),
        "Expenses Included In Valuation": ("expense", "operating_expense", "debit", False),
    },
}

# Prefix defaults for untyped accounts (code = number with dots stripped).
PREFIX_RULES = {
    "Aktiva": (("", ("asset", "current_asset", "debit", False)),),
    # 22xx are long-term liabilities; everything else under Passiva is current.
    "Passiva": (
        ("22", ("liability", "non_current_liability", "credit", False)),
        ("", ("liability", "current_liability", "credit", False)),
    ),
    # 32xx are profit accounts (retained earnings); 31xx are paid-in capital.
    "Modal": (
        ("32", ("equity", "retained_earnings", "credit", False)),
        ("", ("equity", "paid_in_capital", "credit", False)),
    ),
    # 42xx COGS, 44xx other income, 41/43xx operating revenue.
    "Penjualan": (
        ("42", ("cogs", "direct_cost", "debit", False)),
        ("44", ("other_income", "operating_revenue", "credit", False)),
        ("", ("revenue", "operating_revenue", "credit", False)),
    ),
    # 55xx other expenses; the rest are operating expenses.
    "Beban": (
        ("55", ("other_expense", "operating_expense", "debit", False)),
        ("", ("expense", "operating_expense", "debit", False)),
    ),
}


def classify(root, code, erp_type):
    if code in OVERRIDES:
        return OVERRIDES[code], f"override:{code}"
    mapped = ERP_TYPE_MAP.get(root, {}).get(erp_type or "")
    if mapped:
        return mapped, f"erpnext:{erp_type}"
    for prefix, rule in PREFIX_RULES[root]:
        if code.startswith(prefix):
            return rule, f"prefix:{root}:{prefix or 'default'}"
    raise AssertionError(f"unclassified {root}/{code}")


def main():
    src = json.loads(SOURCE.read_text())
    accounts = []
    rule_use = {}
    sort = 0

    def walk(node, root, parent_code):
        nonlocal sort
        for name, val in node.items():
            children = {k: v for k, v in val.items() if isinstance(v, dict)}
            scalars = {k: v for k, v in val.items() if not isinstance(v, dict)}
            number = scalars.get("account_number")
            if not number:
                sys.exit(f"node without account_number: {root}/{name}")
            code = number.replace(".", "")
            (t, sub, bal, rec), rule = classify(root, code, scalars.get("account_type"))
            rule_use[rule] = rule_use.get(rule, 0) + 1
            sort += 1
            accounts.append(
                {
                    "number": number,
                    "code": code,
                    "name": name.strip(),
                    "account_type": t,
                    "account_subtype": sub,
                    "normal_balance": bal,
                    **({"parent_code": parent_code} if parent_code else {}),
                    "is_reconcilable": rec,
                    "currency": "IDR",
                    "sort_order": sort,
                }
            )
            walk(children, root, code)

    for root_name in src["tree"]:
        walk({root_name: src["tree"][root_name]}, root_name, None)

    # Structural self-checks before writing anything.
    seen_codes = set()
    for a in accounts:
        if a["code"] in seen_codes:
            sys.exit(f"duplicate code {a['code']}")
        seen_codes.add(a["code"])
    emitted = set()
    for a in accounts:
        if a.get("parent_code") and a["parent_code"] not in emitted:
            sys.exit(f"parent {a['parent_code']} not emitted before {a['code']}")
        emitted.add(a["code"])

    dataset = {
        "code": DATASET_CODE,
        "version": DATASET_VERSION,
        "name": DATASET_NAME,
        "accounts": accounts,
    }
    TARGET.write_text(json.dumps(dataset, ensure_ascii=False, indent=1) + "\n")

    leaves = sum(1 for a in accounts if not any(x.get("parent_code") == a["code"] for x in accounts))
    print(f"wrote {TARGET}: {len(accounts)} accounts ({leaves} leaves)")
    print("rule use:", dict(sorted(rule_use.items())))


if __name__ == "__main__":
    main()
