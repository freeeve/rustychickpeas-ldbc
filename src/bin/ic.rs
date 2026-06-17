//! LDBC SNB Interactive (IC) workload over real SF data. Thin entry point; the
//! seeds, queries, and runner live in `rustychickpeas_ldbc::interactive`.

// With `--features alloc-count`, count allocations so `--alloc` can attribute
// allocs/bytes per query. Default builds keep the system allocator for pristine
// timing.
#[cfg(feature = "alloc-count")]
#[global_allocator]
static GLOBAL: rustychickpeas_ldbc::alloc_count::CountingAlloc =
    rustychickpeas_ldbc::alloc_count::CountingAlloc;

fn main() -> rustychickpeas_ldbc::Result<()> {
    rustychickpeas_ldbc::interactive::run()
}
