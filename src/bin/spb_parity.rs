//! SPB parity runner: load the SPB-10 extract and emit every feasible query's
//! full result set to `results/spb.parity.rust.json` for the Oxigraph
//! cross-check (`scripts/spb_parity.py`), then print per-query timings and
//! allocation counts. Thin entry point; the work lives in
//! `rustychickpeas_ldbc::spb::parity`.

/// Count allocations so the timing table can report per-query allocs/bytes.
#[global_allocator]
static GLOBAL: rustychickpeas_ldbc::alloc_count::CountingAlloc =
    rustychickpeas_ldbc::alloc_count::CountingAlloc;

fn main() -> rustychickpeas_ldbc::Result<()> {
    rustychickpeas_ldbc::spb::parity::run()
}
