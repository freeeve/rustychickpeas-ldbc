//! Regression tests for the 12 FinBench complex reads (TCR1–TCR12).
//!
//! Each query is asserted against a hand-built synthetic FinBench graph small
//! enough to trace by hand, so the *exact* result (not just non-emptiness) is
//! pinned — this is what guards against the kind of claim-order / off-by-one
//! drift an optimization pass can introduce. The fixture is built straight
//! through `GraphBuilder` (the loader's own node order: accounts, then persons,
//! company, medium, loan), so the internal `NodeId`s are the constants below.

use rustychickpeas_core::{GraphBuilder, GraphSnapshot, PropertyValue};
use rustychickpeas_ldbc::finbench::{self, TruncationOrder};

// NodeIds — assigned in add order (accounts 0..7, persons 8-9, company 10,
// medium 11, loan 12).
const A_HUB: u32 = 0; // P0 owns it; CR1/CR2 reverse target, CR3/CR5 forward src
const A_MID: u32 = 1;
const A_UP: u32 = 2; // blocked-medium signed, loan-deposited; CR9 seed
const A_F1: u32 = 3; // forward chain / 3-cycle / CR7 seed
const A_F2: u32 = 4; // CR6 card
const A_F3: u32 = 5;
const A_C: u32 = 6; // company-owned (CR12 target)
const A_P: u32 = 7; // P1 owns it (CR12 source)
const P0: u32 = 8;
const P1: u32 = 9;
const C0: u32 = 10;
const M0: u32 = 11; // blocked medium
const L0: u32 = 12;

const WMIN: i64 = i64::MIN;
const WMAX: i64 = i64::MAX;
const NO_TRUNC: u32 = 100;

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "expected ~{b}, got {a}");
}

/// Build the synthetic graph. See each test for the expected trace.
fn fixture() -> GraphSnapshot {
    let mut b = GraphBuilder::new(Some(64), Some(64));

    for a in [A_HUB, A_MID, A_UP, A_F1, A_F2, A_F3, A_C, A_P] {
        b.add_node(Some(a), &["Account"]).unwrap();
    }
    b.add_node(Some(P0), &["Person"]).unwrap();
    b.add_node(Some(P1), &["Person"]).unwrap();
    b.add_node(Some(C0), &["Company"]).unwrap();
    b.add_node(Some(M0), &["Medium"]).unwrap();
    b.set_prop_bool(M0, "blocked", true).unwrap();
    b.add_node(Some(L0), &["Loan"]).unwrap();
    b.set_prop_f64(L0, "amount", 7000.0).unwrap();
    b.set_prop_f64(L0, "balance", 3000.0).unwrap();

    let amt = |b: &mut GraphBuilder, u, w, rel, ts: i64, amt: f64| {
        let idx = b.add_relationship(u, w, rel).unwrap();
        b.set_relationship_props_by_index(
            idx,
            &[
                ("ts", PropertyValue::Integer(ts)),
                ("amt", PropertyValue::Float(amt)),
            ],
        );
    };
    let ts = |b: &mut GraphBuilder, u, w, rel, ts: i64| {
        let idx = b.add_relationship(u, w, rel).unwrap();
        b.set_relationship_props_by_index(idx, &[("ts", PropertyValue::Integer(ts))]);
    };

    // transfers (Account -> Account)
    amt(&mut b, A_MID, A_HUB, "transfer", 300, 800.0); // reverse chain into the hub
    amt(&mut b, A_UP, A_MID, "transfer", 200, 800.0);
    amt(&mut b, A_HUB, A_F1, "transfer", 100, 500.0); // forward chain out of the hub
    amt(&mut b, A_F1, A_F2, "transfer", 150, 2000.0);
    amt(&mut b, A_F2, A_F3, "transfer", 160, 2000.0);
    amt(&mut b, A_F3, A_F1, "transfer", 170, 2000.0); // closes the 3-cycle (150<160<170)
    amt(&mut b, A_F3, A_UP, "transfer", 50, 400.0); // A_UP's transfer-in (CR9 rel4)
    amt(&mut b, A_P, A_C, "transfer", 180, 900.0); // person-owned -> company-owned (CR12)
                                                   // withdraw (Account -> Account)
    amt(&mut b, A_F2, A_F3, "withdraw", 200, 1500.0); // CR6 card withdrawal
                                                      // deposit (Loan -> Account)
    amt(&mut b, L0, A_UP, "deposit", 120, 1000.0);
    // repay (Account -> Loan)
    amt(&mut b, A_UP, L0, "repay", 130, 300.0);
    // apply (Person -> Loan), amount carries the loanAmount
    amt(&mut b, P1, L0, "apply", 140, 5000.0);
    // guarantee (Person -> Person)
    ts(&mut b, P0, P1, "guarantee", 140);
    // own (Person/Company -> Account)
    ts(&mut b, P0, A_HUB, "own", 10);
    ts(&mut b, C0, A_C, "own", 10);
    ts(&mut b, P1, A_P, "own", 10);
    // invest (Person -> Company)
    ts(&mut b, P0, C0, "invest", 10);
    ts(&mut b, P1, C0, "invest", 10);
    // signIn (Medium -> Account)
    ts(&mut b, M0, A_UP, "signIn", 10);

    b.finalize(None)
}

#[test]
fn cr1_blocked_medium_upstream() {
    let g = fixture();
    // Reverse from A_HUB: A_MID(300) -> A_UP(200) -> A_F3(50); A_UP is signed in
    // by the blocked medium M0 at distance 2.
    let r = finbench::cr1(&g, A_HUB, WMIN, WMAX, NO_TRUNC, false);
    assert_eq!(r, vec![(A_UP, 2, M0, "Medium")]);
}

#[test]
fn cr1_empty_window_returns_nothing() {
    let g = fixture();
    // No transfer is in [1000, 2000], so the reverse trace can't start.
    assert!(finbench::cr1(&g, A_HUB, 1000, 2000, NO_TRUNC, false).is_empty());
}

#[test]
fn cr2_loan_gather() {
    let g = fixture();
    // P0 owns A_HUB; reverse trace reaches {A_MID, A_UP, A_F3}; only A_UP has an
    // incoming deposit (from L0: amount 7000, balance 3000).
    let r = finbench::cr2(&g, P0, WMIN, WMAX, NO_TRUNC, false);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].0, A_UP);
    approx(r[0].1, 7000.0);
    approx(r[0].2, 3000.0);
}

#[test]
fn cr3_shortest_transfer_path() {
    let g = fixture();
    // A_HUB -> A_F1 -> A_F2 -> A_F3 is the only forward route: 3 hops.
    assert_eq!(
        finbench::shortest_transfer_path(&g, A_HUB, A_F3, WMIN, WMAX),
        3
    );
    // A_C has no outgoing transfers, so nothing is reachable from it.
    assert_eq!(
        finbench::shortest_transfer_path(&g, A_C, A_HUB, WMIN, WMAX),
        -1
    );
}

#[test]
fn cr4_three_cycle() {
    let g = fixture();
    // A_F1 -> A_F2 -> A_F3 -> A_F1 with strictly increasing ts (150<160<170).
    let cycles = finbench::transfer_cycles(&g, A_F1, 1000.0, 1000);
    assert_eq!(cycles, vec![vec![A_F1, A_F2, A_F3]]);
}

#[test]
fn cr5_exact_transfer_trace() {
    let g = fixture();
    // P0 owns A_HUB; ascending-ts forward traces (<=3 hops): [hub,f1],
    // [hub,f1,f2], [hub,f1,f2,f3]. Sorted by length descending.
    let paths = finbench::cr5(&g, P0, WMIN, WMAX, NO_TRUNC, "desc");
    assert_eq!(paths.len(), 3);
    assert_eq!(paths[0], vec![A_HUB, A_F1, A_F2, A_F3]);
}

#[test]
fn cr6_withdraw_after_many_to_one() {
    let g = fixture();
    // Card A_F2 withdraws 1500 (last at ts 200); its only in-window incoming
    // transfer before that is from A_F1 (2000).
    let r = finbench::cr6(&g, A_F2, 0.0, 0.0, WMIN, WMAX, NO_TRUNC, "desc");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].0, A_F1);
    approx(r[0].1, 2000.0);
    approx(r[0].2, 1500.0);
}

#[test]
fn cr7_in_out_ratio() {
    let g = fixture();
    // A_F1 in: from A_HUB(500) and A_F3(2000) -> 2 sources, 2500 in.
    //          out: to A_F2(2000) -> 1 dest. ratio 2500/2000 = 1.25.
    let (num_src, num_dst, ratio) = finbench::cr7(
        &g,
        A_F1,
        0.0,
        WMIN,
        WMAX,
        NO_TRUNC,
        TruncationOrder::Descending,
    );
    assert_eq!((num_src, num_dst), (2, 1));
    approx(ratio, 1.25);
}

#[test]
fn cr7_truncation_binds() {
    let g = fixture();
    // Limit 1, descending by time: of A_F1's two incoming transfers keep only the
    // newest (A_F3 @ ts170, 2000). ratio 2000/2000 = 1.0.
    let (num_src, num_dst, ratio) =
        finbench::cr7(&g, A_F1, 0.0, WMIN, WMAX, 1, TruncationOrder::Descending);
    assert_eq!((num_src, num_dst), (1, 1));
    approx(ratio, 1.0);
}

#[test]
fn cr8_transfer_trace_after_loan() {
    let g = fixture();
    // L0 deposits A_UP(d1); A_UP->A_MID(d2)->A_HUB(d3).
    let r = finbench::cr8(&g, L0, 0.0, WMIN, WMAX, NO_TRUNC, "DESC");
    let dist: std::collections::HashMap<u32, u32> = r.iter().map(|&(id, _, d)| (id, d)).collect();
    assert_eq!(dist.len(), 3);
    assert_eq!(dist[&A_UP], 1);
    assert_eq!(dist[&A_MID], 2);
    assert_eq!(dist[&A_HUB], 3);
}

#[test]
fn cr9_laundering_ratios() {
    let g = fixture();
    // A_UP: repay 300 (e1), deposit-in 1000 (e2), transfer-out 800 (e3),
    // transfer-in 400 (e4). repay=300/1000=0.3, deposit=300/400=0.75,
    // transfer=800/400=2.0.
    let (repay, deposit, transfer) = finbench::cr9(&g, A_UP, 0.0, WMIN, WMAX, 100, false);
    approx(repay as f64, 0.3);
    approx(deposit as f64, 0.75);
    approx(transfer as f64, 2.0);
}

#[test]
fn cr10_investor_similarity() {
    let g = fixture();
    // P0 and P1 both invest in C0; P0's only co-investor is P1, sharing 1 company.
    let r = finbench::cr10(&g, P0, WMIN, WMAX);
    assert_eq!(r, vec![(P1, 1)]);
}

#[test]
fn cr11_guarantee_exposure() {
    let g = fixture();
    // P0 guarantees P1; P1 applied for L0 (5000). P0's own applications: none.
    approx(finbench::guarantee_exposure(&g, P0), 5000.0);
}

#[test]
fn cr12_company_transfer() {
    let g = fixture();
    // P1 owns A_P; A_P transfers 900 to A_C, which C0 (a Company) owns.
    let r = finbench::cr12(&g, P1, WMIN, WMAX, NO_TRUNC, TruncationOrder::Descending);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].0, A_C);
    approx(r[0].1, 900.0);
}
