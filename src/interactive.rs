//! SCAFFOLD — SNB Interactive (IC) workload. Not yet wired into the build.
//!
//! Cargo only compiles modules reachable from a crate root; this file is not
//! declared with `mod interactive;` anywhere yet, so it is inert until
//! `tasks/001` extracts `src/lib.rs` and `tasks/003` turns these stubs into a
//! `src/bin/ic.rs` binary. Signatures assume the shared lib will re-export the
//! loader + helpers currently living in `main.rs`.
//!
//! IC is seed-anchored short reads, reusing the same `initial_snapshot` BI
//! loads. Each `ic*` takes the loaded `GraphSnapshot` plus the query's seed
//! parameters (a person id, a date, a name) and returns its result rows.
//!
//! Reuse map (capabilities already in `main.rs`):
//!   * IC1/IC9  — bounded `knows` BFS            ≈ `knows_reachability` shape
//!   * IC13     — unweighted shortest path        ≈ `g.dijkstra(.., |_, _| 1.0)`
//!   * IC14     — weighted shortest path          ≈ Q19/Q20 `g.dijkstra` + weight
//!   * IC11/IC12— org / tagclass traversals       ≈ Q20 `workAt` / Q10 experts

use rustychickpeas_core::GraphSnapshot;

/// IC1 — friends (transitive `knows`, ≤3 hops) with a given first name, ordered
/// by distance then lastName, with their workplaces/universities/profile.
pub fn ic1_friends_by_name(_g: &GraphSnapshot, _person: u32, _first_name: &str) -> Vec<u32> {
    todo!("bounded knows BFS to depth 3, filter firstName, hydrate profile")
}

/// IC2 — the 20 most recent messages from the seed person's friends created
/// before a max date.
pub fn ic2_recent_messages(_g: &GraphSnapshot, _person: u32, _max_day: i64) -> Vec<u32> {
    todo!("1-hop knows -> hasCreator^-1 messages, filter date, top-20 by date")
}

/// IC3 — friends/friends-of-friends who made messages in both of two countries
/// within a window.
pub fn ic3_two_countries(
    _g: &GraphSnapshot,
    _person: u32,
    _country_a: &str,
    _country_b: &str,
    _start_day: i64,
    _window_days: i64,
) -> Vec<u32> {
    todo!("2-hop knows, isLocatedIn country filter, count per-country in window")
}

/// IC9 — the 20 most recent messages from the seed's friends-of-friends.
pub fn ic9_fof_messages(_g: &GraphSnapshot, _person: u32, _max_day: i64) -> Vec<u32> {
    todo!("2-hop knows, hasCreator^-1, top-20 by date")
}

/// IC13 — unweighted shortest path length in the `knows` graph between two
/// persons (-1 if unreachable). Mirrors Q19 minus the interaction weights.
pub fn ic13_shortest_path(_g: &GraphSnapshot, _p1: u32, _p2: u32) -> i64 {
    todo!("g.dijkstra(p1, Direction::Both, &[\"knows\"], Some(p2), |_, _| 1.0)")
}

/// IC14 — all weighted shortest paths between two persons; edge weight derived
/// from reply/like interactions. Mirrors Q19/Q20 weighting.
pub fn ic14_weighted_paths(_g: &GraphSnapshot, _p1: u32, _p2: u32) -> Vec<(Vec<u32>, f64)> {
    todo!("interaction weight map (cf. build_interaction_map) + g.dijkstra paths")
}

// Short reads IS1–IS7 (single-hop profile/message lookups) and the IC4–IC8 /
// IC10–IC12 complex reads are filled in by tasks/003; list which are feasible
// with the currently-loaded schema vs. needing Forum/Organisation edges.
