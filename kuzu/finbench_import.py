#!/usr/bin/env python3
"""Import a FinBench raw/ directory into a Kùzu database for the head-to-head
reference (tasks/078).

FinBench CSV is pipe-delimited with a free-text `comment` column (prose that can
contain quotes/commas), so each file is trimmed to just the columns the queries
need — read with QUOTE_NONE and only the early columns, which sit before any
prose — then COPYed into a minimal schema matching the Rust loader's node/rel
set (so node/rel counts line up).

    .venv-kuzu/bin/python kuzu/finbench_import.py [raw_dir] [db_path]
"""
import csv
import glob
import os
import shutil
import sys
import tempfile
import time

import kuzu

RAW = sys.argv[1] if len(sys.argv) > 1 else "data/finbench/raw"
DBPATH = sys.argv[2] if len(sys.argv) > 2 else "kuzu/db-finbench"

# (subdir, table, [(prop, src_col, kuzu_type)]); first entry is the PRIMARY KEY.
NODES = [
    ("account", "Account", [("id", "id", "INT64"), ("isBlocked", "isBlocked", "BOOLEAN")]),
    ("person", "Person", [("id", "id", "INT64")]),
    ("company", "Company", [("id", "id", "INT64")]),
    ("loan", "Loan", [("id", "id", "INT64"), ("loanAmount", "loanAmount", "DOUBLE"), ("balance", "balance", "DOUBLE")]),
    ("medium", "Medium", [("id", "id", "INT64"), ("isBlocked", "isBlocked", "BOOLEAN")]),
]
# (subdir, table, FROM, TO, from_col, to_col, [(prop, src_col, type)])
RELS = [
    ("transfer", "transfer", "Account", "Account", "fromId", "toId", [("createTime", "createTime", "INT64"), ("amount", "amount", "DOUBLE")]),
    ("withdraw", "withdraw", "Account", "Account", "fromId", "toId", [("createTime", "createTime", "INT64"), ("amount", "amount", "DOUBLE")]),
    ("deposit", "deposit", "Loan", "Account", "loanId", "accountId", [("createTime", "createTime", "INT64"), ("amount", "amount", "DOUBLE")]),
    ("repay", "repay", "Account", "Loan", "accountId", "loanId", [("createTime", "createTime", "INT64"), ("amount", "amount", "DOUBLE")]),
    ("signIn", "signIn", "Medium", "Account", "mediumId", "accountId", [("createTime", "createTime", "INT64")]),
    ("personGuarantee", "personGuarantee", "Person", "Person", "fromId", "toId", [("createTime", "createTime", "INT64")]),
    ("companyGuarantee", "companyGuarantee", "Company", "Company", "fromId", "toId", [("createTime", "createTime", "INT64")]),
    ("personApplyLoan", "personApply", "Person", "Loan", "personId", "loanId", [("createTime", "createTime", "INT64"), ("loanAmount", "loanAmount", "DOUBLE")]),
    ("companyApplyLoan", "companyApply", "Company", "Loan", "companyId", "loanId", [("createTime", "createTime", "INT64"), ("loanAmount", "loanAmount", "DOUBLE")]),
    ("personOwnAccount", "personOwn", "Person", "Account", "personId", "accountId", [("createTime", "createTime", "INT64")]),
    ("companyOwnAccount", "companyOwn", "Company", "Account", "companyId", "accountId", [("createTime", "createTime", "INT64")]),
    ("personInvest", "personInvest", "Person", "Company", "investorId", "companyId", [("createTime", "createTime", "INT64")]),
    ("companyInvest", "companyInvest", "Company", "Company", "investorId", "companyId", [("createTime", "createTime", "INT64")]),
]


def trim(subdir, src_cols):
    """Write a comma CSV (no header) of `src_cols` from every part file; returns
    (path, rows) or None if the directory is absent/empty."""
    files = sorted(glob.glob(os.path.join(RAW, subdir, "*.csv")))
    if not files:
        return None
    fd, out = tempfile.mkstemp(suffix=".csv")
    os.close(fd)
    rows = 0
    with open(out, "w", newline="") as w:
        wr = csv.writer(w)
        for f in files:
            with open(f, newline="") as r:
                rd = csv.reader(r, delimiter="|", quoting=csv.QUOTE_NONE)
                header = next(rd)
                idx = [header.index(c) for c in src_cols]
                for row in rd:
                    wr.writerow([row[i] for i in idx])
                    rows += 1
    return out, rows


def main():
    if os.path.exists(DBPATH):
        shutil.rmtree(DBPATH)
    conn = kuzu.Connection(kuzu.Database(DBPATH))
    t0 = time.time()
    n_nodes = n_rels = 0

    for subdir, table, cols in NODES:
        coldef = ", ".join(f"{p} {t}" for p, _, t in cols)
        conn.execute(f"CREATE NODE TABLE {table}({coldef}, PRIMARY KEY({cols[0][0]}))")
        res = trim(subdir, [src for _, src, _ in cols])
        if res:
            path, rows = res
            conn.execute(f"COPY {table} FROM '{path}' (HEADER=false)")
            os.remove(path)
            n_nodes += rows
            print(f"  node {table}: {rows}")

    for subdir, table, frm, to, fc, tc, props in RELS:
        propdef = "".join(f", {p} {t}" for p, _, t in props)
        conn.execute(f"CREATE REL TABLE {table}(FROM {frm} TO {to}{propdef})")
        res = trim(subdir, [fc, tc] + [src for _, src, _ in props])
        if res:
            path, rows = res
            conn.execute(f"COPY {table} FROM '{path}' (HEADER=false)")
            os.remove(path)
            n_rels += rows
            print(f"  rel  {table}: {rows}")

    print(f"Kùzu FinBench loaded: {n_nodes} nodes, {n_rels} rels -> {DBPATH} [{time.time() - t0:.1f}s]")


if __name__ == "__main__":
    main()
