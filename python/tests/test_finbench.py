"""Tests for the Python FinBench queries on the hand-built synthetic fixture
(ported from tests/finbench_queries.rs, same node ids assigned in add order)."""

from finbench import queries as fb
from rustychickpeas import GraphSnapshotBuilder

WMIN = -(2 ** 63)
WMAX = 2 ** 63 - 1

# NodeIds — accounts 0..7, persons 8-9, company 10, medium 11, loan 12.
A_HUB, A_MID, A_UP, A_F1, A_F2, A_F3, A_C, A_P = range(8)
P0, P1, C0, M0, L0 = 8, 9, 10, 11, 12


def _fixture():
    b = GraphSnapshotBuilder()
    for a in range(8):
        b.add_node(["Account"], node_id=a)
    b.add_node(["Person"], node_id=P0)
    b.add_node(["Person"], node_id=P1)
    b.add_node(["Company"], node_id=C0)
    b.add_node(["Medium"], node_id=M0)
    b.set_prop(M0, "blocked", True)
    b.add_node(["Loan"], node_id=L0)
    b.set_prop(L0, "amount", 7000.0)
    b.set_prop(L0, "balance", 3000.0)

    def amt(u, w, rel, ts, a):
        b.add_relationship(u, w, rel)
        b.set_relationship_prop_i64(u, w, rel, "ts", ts)
        b.set_relationship_prop_f64(u, w, rel, "amt", a)

    def ts(u, w, rel, t):
        b.add_relationship(u, w, rel)
        b.set_relationship_prop_i64(u, w, rel, "ts", t)

    amt(A_MID, A_HUB, "transfer", 300, 800.0)   # reverse chain into the hub
    amt(A_UP, A_MID, "transfer", 200, 800.0)
    amt(A_HUB, A_F1, "transfer", 100, 500.0)    # forward chain out of the hub
    amt(A_F1, A_F2, "transfer", 150, 2000.0)
    amt(A_F2, A_F3, "transfer", 160, 2000.0)
    amt(A_F3, A_F1, "transfer", 170, 2000.0)    # closes the 3-cycle (150<160<170)
    amt(A_F3, A_UP, "transfer", 50, 400.0)
    amt(A_P, A_C, "transfer", 180, 900.0)       # person-owned -> company-owned
    amt(A_F2, A_F3, "withdraw", 200, 1500.0)
    amt(L0, A_UP, "deposit", 120, 1000.0)
    amt(A_UP, L0, "repay", 130, 300.0)
    amt(P1, L0, "apply", 140, 5000.0)
    ts(P0, P1, "guarantee", 140)
    ts(P0, A_HUB, "own", 10)
    ts(C0, A_C, "own", 10)
    ts(P1, A_P, "own", 10)
    ts(P0, C0, "invest", 10)
    ts(P1, C0, "invest", 10)
    ts(M0, A_UP, "signIn", 10)
    return b.finalize()


NO_TRUNC = 100


def test_cr1_blocked_medium_upstream():
    g = _fixture()
    # Reverse from A_HUB: A_MID(300) -> A_UP(200) -> A_F3(50); A_UP is signed in by
    # the blocked medium M0 at distance 2.
    assert fb.cr1(g, A_HUB, WMIN, WMAX, NO_TRUNC, False) == [(A_UP, 2, M0, "Medium")]
    # No transfer is in [1000, 2000], so the reverse trace can't start.
    assert fb.cr1(g, A_HUB, 1000, 2000, NO_TRUNC, False) == []


def test_cr2_loan_gather():
    g = _fixture()
    # P0 owns A_HUB; reverse trace reaches {A_MID, A_UP, A_F3}; only A_UP has an
    # incoming deposit (from L0: amount 7000, balance 3000).
    r = fb.cr2(g, P0, WMIN, WMAX, NO_TRUNC, False)
    assert r == [(A_UP, 7000.0, 3000.0)]


def test_cr3_shortest_transfer_path():
    g = _fixture()
    # A_HUB -> A_F1 -> A_F2 -> A_F3 is the only forward route: 3 hops.
    assert fb.shortest_transfer_path(g, A_HUB, A_F3, WMIN, WMAX) == 3
    # A_C has no outgoing transfers -> nothing reachable.
    assert fb.shortest_transfer_path(g, A_C, A_HUB, WMIN, WMAX) == -1


def test_cr4_three_cycle():
    g = _fixture()
    # A_F1 -> A_F2 -> A_F3 -> A_F1 with strictly increasing ts (150<160<170).
    assert fb.transfer_cycles(g, A_F1, 1000.0, 1000) == [[A_F1, A_F2, A_F3]]


def test_cr5_exact_transfer_trace():
    g = _fixture()
    # P0 owns A_HUB; ascending-ts forward traces: [hub,f1], [hub,f1,f2],
    # [hub,f1,f2,f3]. Sorted by length descending.
    paths = fb.cr5(g, P0, WMIN, WMAX, NO_TRUNC, "desc")
    assert len(paths) == 3
    assert paths[0] == [A_HUB, A_F1, A_F2, A_F3]


def test_cr6_withdraw_after_many_to_one():
    g = _fixture()
    # Card A_F2 withdraws 1500 (last ts 200); its only in-window incoming transfer
    # before that is from A_F1 (2000).
    assert fb.cr6(g, A_F2, 0.0, 0.0, WMIN, WMAX, NO_TRUNC, "desc") == [(A_F1, 2000.0, 1500.0)]


def test_cr7_in_out_ratio():
    g = _fixture()
    # A_F1 in: A_HUB(500) + A_F3(2000) = 2 sources, 2500; out: A_F2(2000) = 1 dest.
    # ratio 2500/2000 = 1.25.
    assert fb.cr7(g, A_F1, 0.0, WMIN, WMAX, NO_TRUNC, False) == (2, 1, 1.25)
    # Limit 1, descending: keep the newest incoming (A_F3 @170, 2000) -> 2000/2000 = 1.0.
    assert fb.cr7(g, A_F1, 0.0, WMIN, WMAX, 1, False) == (1, 1, 1.0)


def test_cr8_transfer_trace_after_loan():
    g = _fixture()
    # L0 deposits A_UP (d1); A_UP -> A_MID (d2) -> A_HUB (d3).
    r = fb.cr8(g, L0, 0.0, WMIN, WMAX, NO_TRUNC, "DESC")
    dist = {did: d for (did, _ratio, d) in r}
    assert dist == {A_UP: 1, A_MID: 2, A_HUB: 3}


def test_cr9_laundering_ratios():
    g = _fixture()
    # A_UP: repay 300, deposit-in 1000, transfer-out 800, transfer-in 400.
    repay, deposit, transfer = fb.cr9(g, A_UP, 0.0, WMIN, WMAX, 100, False)
    assert abs(repay - 0.3) < 1e-6      # 300/1000
    assert abs(deposit - 0.75) < 1e-6   # 300/400
    assert abs(transfer - 2.0) < 1e-6   # 800/400


def test_cr10_investor_similarity():
    g = _fixture()
    # P0 and P1 both invest in C0; P0's only co-investor is P1, sharing 1 company.
    assert fb.cr10(g, P0, WMIN, WMAX) == [(P1, 1)]


def test_cr12_company_transfer():
    g = _fixture()
    # P1 owns A_P; A_P transfers 900 to A_C, which Company C0 owns.
    assert fb.cr12(g, P1, WMIN, WMAX, NO_TRUNC, False) == [(A_C, 900.0)]


def test_cr11_guarantee_exposure():
    g = _fixture()
    # P0 guarantees P1; P1 is on a loan applied for 5000. P0's exposure = 5000.
    assert fb.guarantee_exposure(g, P0) == 5000.0


def test_trace_transfers_in_window():
    g = _fixture()
    # Empty window -> no in-window transfer, nothing reached.
    assert fb.trace_transfers_in(g, A_HUB, 1000, 2000, 10) == []
    # Full window, 1 hop: only the direct incoming transfer A_MID -> A_HUB.
    assert fb.trace_transfers_in(g, A_HUB, WMIN, WMAX, 1) == [A_MID]
