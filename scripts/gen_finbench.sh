#!/bin/bash
# Generate a small LDBC FinBench dataset (financial transaction network).
#
# FinBench is produced by a Spark-based generator (unlike the curl-and-extract BI
# dataset). This script clones + builds the generator and runs the smallest scale
# factor locally. It needs only a JDK (Java 11 works) — Spark is bundled in the
# shaded jar and the generator hardcodes a `local` master, so there is no separate
# Spark install or spark-submit step.
#
#   ./scripts/gen_finbench.sh 0.1 data/finbench
#
# Output: pipe-delimited CSV under <out>/raw with the transaction schema
# (Account/Person/Company/Medium/Loan nodes; transfer/withdraw/deposit/repay/
# guarantee/invest/signIn/own/apply edges, each timestamped + amount-weighted).
# The loader (tasks/007) ingests <out>/raw.
set -euo pipefail

SF="${1:-0.1}"
OUT_DIR="${2:-data/finbench}"
REPO="${FINBENCH_REPO:-https://github.com/ldbc/ldbc_finbench_datagen.git}"
SRC_DIR="${FINBENCH_SRC:-.cache/ldbc_finbench_datagen}"

# Locate a JDK. Homebrew's openjdk@11 is keg-only (off PATH); prefer it, else
# fall back to the system java_home, else an env-supplied JAVA_HOME.
if [[ -z "${JAVA_HOME:-}" ]]; then
    if [[ -x /opt/homebrew/opt/openjdk@11/bin/java ]]; then
        JAVA_HOME="/opt/homebrew/opt/openjdk@11"
    elif /usr/libexec/java_home >/dev/null 2>&1; then
        JAVA_HOME="$(/usr/libexec/java_home)"
    fi
fi
[[ -n "${JAVA_HOME:-}" && -x "${JAVA_HOME}/bin/java" ]] || {
    echo "ERROR: no JDK found. Install one (e.g. 'brew install openjdk@11') or set JAVA_HOME." >&2
    exit 1
}
JAVA="${JAVA_HOME}/bin/java"
export JAVA_HOME PATH="${JAVA_HOME}/bin:${PATH}"
echo "Using JDK: $("${JAVA}" -version 2>&1 | head -1)" >&2

if [[ ! -d "${SRC_DIR}" ]]; then
    echo "Cloning FinBench datagen into ${SRC_DIR} ..." >&2
    git clone --depth 1 "${REPO}" "${SRC_DIR}"
fi

JAR="$(ls "${SRC_DIR}"/target/*-jar-with-dependencies.jar 2>/dev/null | head -1 || true)"
if [[ -z "${JAR}" ]]; then
    echo "Building the generator (mvn clean package -DskipTests) ..." >&2
    ( cd "${SRC_DIR}" && mvn clean package -DskipTests -q )
    JAR="$(ls "${SRC_DIR}"/target/*-jar-with-dependencies.jar | head -1)"
fi

# Spark 3.2 on Java 11 needs the module --add-opens flags that spark-submit
# normally injects; pass them ourselves since we run the shaded jar directly.
ADD_OPENS=(
    --add-opens=java.base/java.lang=ALL-UNNAMED
    --add-opens=java.base/java.lang.invoke=ALL-UNNAMED
    --add-opens=java.base/java.lang.reflect=ALL-UNNAMED
    --add-opens=java.base/java.io=ALL-UNNAMED
    --add-opens=java.base/java.net=ALL-UNNAMED
    --add-opens=java.base/java.nio=ALL-UNNAMED
    --add-opens=java.base/java.util=ALL-UNNAMED
    --add-opens=java.base/java.util.concurrent=ALL-UNNAMED
    --add-opens=java.base/java.util.concurrent.atomic=ALL-UNNAMED
    --add-opens=java.base/sun.nio.ch=ALL-UNNAMED
    --add-opens=java.base/sun.nio.cs=ALL-UNNAMED
    --add-opens=java.base/sun.security.action=ALL-UNNAMED
    --add-opens=java.base/sun.util.calendar=ALL-UNNAMED
)

GEN_OUT="${SRC_DIR}/out"
echo "Generating scale factor ${SF} (Spark local mode) ..." >&2
rm -rf "${GEN_OUT}"
"${JAVA}" -Xmx8g "${ADD_OPENS[@]}" -cp "${JAR}" \
    ldbc.finbench.datagen.LdbcDatagen --scale-factor "${SF}" --output-dir "${GEN_OUT}"

mkdir -p "${OUT_DIR}"
rm -rf "${OUT_DIR}/raw"
cp -r "${GEN_OUT}/raw" "${OUT_DIR}/raw"
echo "Done. Data at ${OUT_DIR}/raw" >&2
echo "Load with: cargo run --release --bin finbench -- ${OUT_DIR}/raw" >&2
