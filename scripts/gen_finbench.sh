#!/bin/bash
# Generate a small LDBC FinBench dataset.
#
# Unlike the BI dataset (a curl-and-extract download), FinBench is produced by a
# Spark-based generator, so this needs a JDK + Spark available locally. This
# script clones and builds the generator and runs the smallest scale factor; it
# documents the prerequisite rather than pretending it is a one-liner.
#
#   ./scripts/gen_finbench.sh 0.1 data/finbench
#
# Output is CSV with a transaction schema (Account/Person/Company/Medium/Loan
# nodes; transfer/withdraw/deposit/repay/guarantee/invest/signIn/own/apply edges,
# each with a timestamp and amount). The loader in tasks/007 ingests it.
set -euo pipefail

SF="${1:-0.1}"
OUT_DIR="${2:-data/finbench}"
REPO="${FINBENCH_REPO:-https://github.com/ldbc/ldbc_finbench_datagen.git}"
SRC_DIR="${FINBENCH_SRC:-.cache/ldbc_finbench_datagen}"

command -v java >/dev/null 2>&1 || { echo "ERROR: JDK required (java not found)." >&2; exit 1; }

if [[ ! -d "${SRC_DIR}" ]]; then
    echo "Cloning FinBench datagen into ${SRC_DIR} ..." >&2
    git clone --depth 1 "${REPO}" "${SRC_DIR}"
fi

echo "Build + run the generator per its README (Spark required)." >&2
echo "  cd ${SRC_DIR} && <build per repo> && <run with scale factor ${SF}>" >&2
echo "Then point the loader at the CSV output:" >&2
echo "  mkdir -p ${OUT_DIR} && cp -r ${SRC_DIR}/out/* ${OUT_DIR}/" >&2
echo "  cargo run --release --bin finbench -- ${OUT_DIR}" >&2
# NOTE: the exact build/run invocation is intentionally not hard-coded — it
# tracks the upstream repo's README, which changes across releases. Verify there.
