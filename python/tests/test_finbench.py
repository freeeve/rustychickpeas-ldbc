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
