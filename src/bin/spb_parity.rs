//! SPB parity runner: load the SPB-10 extract and emit every feasible query's
//! full result set to `results/spb.parity.rust.json` for the Oxigraph
//! cross-check (`scripts/spb_parity.py`). Thin entry point; the work lives in
//! `rustychickpeas_ldbc::spb::parity`.

fn main() -> rustychickpeas_ldbc::Result<()> {
    rustychickpeas_ldbc::spb::parity::run()
}
