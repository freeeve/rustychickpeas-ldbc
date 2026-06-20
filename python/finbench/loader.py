"""Load an LDBC FinBench ``raw/`` directory into a rustychickpeas GraphSnapshot,
mirroring src/finbench.rs::load_finbench.

The financial graph: Account / Person / Company / Medium / Loan nodes and the
transfer/withdraw/deposit/repay/apply/guarantee/own/invest/signIn rels. createTime
is already epoch-ms, so it loads straight into the ``ts`` rel property; amount goes
to ``amt``. FinBench ids are unique only within a type, so refs are resolved by the
``id`` property scoped to the endpoint's label.
"""

import csv
import glob
import os
import tempfile

from rustychickpeas import GraphSnapshotBuilder, Ref, Rel, Prop


def _parts(entity_dir):
    return sorted(glob.glob(os.path.join(entity_dir, "part-*.csv")))


def _iter_rows(entity_dir, cols):
    for path in _parts(entity_dir):
        with open(path, newline="", encoding="utf-8") as fh:
            reader = csv.reader(fh, delimiter="|")
            header = next(reader)
            idx = [header.index(c) for c in cols]
            for row in reader:
                yield [row[i] for i in idx]


def _load_simple(b, entity_dir, label):
    """Load id-only nodes (Person/Company), straight off the pipe-delimited parts."""
    n = 0
    for path in _parts(entity_dir):
        n += len(b.load_nodes_from_csv(path, property_columns=["id"],
                                       delimiter="|", default_label=label))
    return n


def _load_normalized(b, entity_dir, out_path, label, src_cols, header, derive):
    """Normalize the needed columns to a comma-CSV (deriving typed props) then load."""
    n = 0
    with open(out_path, "w", newline="", encoding="utf-8") as out:
        w = csv.writer(out)
        w.writerow(header)
        for row in _iter_rows(entity_dir, src_cols):
            w.writerow(derive(row))
            n += 1
    b.load_nodes_from_csv(out_path, property_columns=header, default_label=label)
    return n


def _amt_rel(rel, fc, tc, from_label, to_label, amt_col):
    return Rel(rel, Ref(fc, from_label), Ref(tc, to_label),
               props=[Prop("ts", "createTime", int), Prop("amt", amt_col, float)])


def _ts_rel(rel, fc, tc, from_label, to_label):
    return Rel(rel, Ref(fc, from_label), Ref(tc, to_label),
               props=[Prop("ts", "createTime", int)])


def _load_rels(b, entity_dir, rel_spec):
    n = 0
    for path in _parts(entity_dir):
        n += sum(b.load_relationships_from_csv_multi(path, [rel_spec], delimiter="|"))
    return n


def load_finbench(raw_path):
    """Build the FinBench graph from a ``raw/`` dir. Returns (snapshot, stats)."""
    b = GraphSnapshotBuilder(capacity_nodes=1_200_000, capacity_rels=10_000_000)
    s = {}
    with tempfile.TemporaryDirectory() as tmp:
        # --- nodes (Rust load order: account, person, company, medium, loan) ---
        s["accounts"] = _load_normalized(
            b, f"{raw_path}/account", os.path.join(tmp, "account.csv"), "Account",
            ["id", "isBlocked"], ["id", "blocked"],
            lambda r: [r[0], 1 if r[1] == "true" else 0])
        s["persons"] = _load_simple(b, f"{raw_path}/person", "Person")
        s["companies"] = _load_simple(b, f"{raw_path}/company", "Company")
        s["media"] = _load_normalized(
            b, f"{raw_path}/medium", os.path.join(tmp, "medium.csv"), "Medium",
            ["id", "isBlocked"], ["id", "blocked"],
            lambda r: [r[0], 1 if r[1] == "true" else 0])
        s["loans"] = _load_normalized(
            b, f"{raw_path}/loan", os.path.join(tmp, "loan.csv"), "Loan",
            ["id", "loanAmount", "balance"], ["id", "amount", "balance"],
            lambda r: [r[0], r[1], r[2]])

        # --- rels (ts always; amt where the schema carries an amount) ---
        rels = 0
        rels += _load_rels(b, f"{raw_path}/transfer", _amt_rel("transfer", "fromId", "toId", "Account", "Account", "amount"))
        rels += _load_rels(b, f"{raw_path}/withdraw", _amt_rel("withdraw", "fromId", "toId", "Account", "Account", "amount"))
        rels += _load_rels(b, f"{raw_path}/deposit", _amt_rel("deposit", "loanId", "accountId", "Loan", "Account", "amount"))
        rels += _load_rels(b, f"{raw_path}/repay", _amt_rel("repay", "accountId", "loanId", "Account", "Loan", "amount"))
        rels += _load_rels(b, f"{raw_path}/personApplyLoan", _amt_rel("apply", "personId", "loanId", "Person", "Loan", "loanAmount"))
        rels += _load_rels(b, f"{raw_path}/companyApplyLoan", _amt_rel("apply", "companyId", "loanId", "Company", "Loan", "loanAmount"))
        rels += _load_rels(b, f"{raw_path}/personGuarantee", _ts_rel("guarantee", "fromId", "toId", "Person", "Person"))
        rels += _load_rels(b, f"{raw_path}/companyGuarantee", _ts_rel("guarantee", "fromId", "toId", "Company", "Company"))
        rels += _load_rels(b, f"{raw_path}/personOwnAccount", _ts_rel("own", "personId", "accountId", "Person", "Account"))
        rels += _load_rels(b, f"{raw_path}/companyOwnAccount", _ts_rel("own", "companyId", "accountId", "Company", "Account"))
        rels += _load_rels(b, f"{raw_path}/personInvest", _ts_rel("invest", "investorId", "companyId", "Person", "Company"))
        rels += _load_rels(b, f"{raw_path}/companyInvest", _ts_rel("invest", "investorId", "companyId", "Company", "Company"))
        rels += _load_rels(b, f"{raw_path}/signIn", _ts_rel("signIn", "mediumId", "accountId", "Medium", "Account"))
        s["rels"] = rels

    return b.finalize(), s
