"""FinBench complex-read queries — temporal traversals over the financial graph.

Rel props (ts/amt) are read in bulk via the native accessors instead of per-rel
Relationship objects:
  - rels_with_props(node, dir, rel, keys) -> (neighbors, [value_lists])  [filter/traverse]
  - rel_view(node, dir, rel, keys).col(key) -> buffer; sum(memoryview(...)) is C-speed
    [reductions]
Mirrors src/finbench.rs.
"""

from collections import deque

from rustychickpeas import Direction

_I64_MIN = -(2 ** 63)
_I64_MAX = 2 ** 63 - 1
MAX_CYCLE_LEN = 6
MAX_CYCLES = 1000


def trace_transfers_in(g, account, start_ms, end_ms, max_hops):
    """Upstream accounts feeding into ``account`` by a <=max_hops, in-window reverse
    ``transfer`` BFS (TCR1's reverse trace)."""
    visited = {account}
    queue = deque([(account, 0)])
    reached = []
    while queue:
        node, depth = queue.popleft()
        if depth >= max_hops:
            continue
        nbrs, (ts,) = g.rels_with_props(node, Direction.Incoming, "transfer", ["ts"])
        for nbr, t in zip(nbrs, ts):
            if start_ms <= t <= end_ms and nbr not in visited:
                visited.add(nbr)
                reached.append(nbr)
                queue.append((nbr, depth + 1))
    return reached


def transfer_cycles(g, account, min_amount, window_ms):
    """TCR8 — fund-transfer cycles back to ``account`` where each hop is strictly
    later in time, each amount >= ``min_amount``, completing within ``window_ms``."""
    cycles = []
    _cycle_dfs(g, account, account, _I64_MIN, None, min_amount, window_ms, [account], {account}, cycles)
    return cycles


def _cycle_dfs(g, start, node, last_ts, first_ts, min_amount, window_ms, path, on_path, out):
    if len(path) > MAX_CYCLE_LEN or len(out) >= MAX_CYCLES:
        return
    nbrs, (tss, amts) = g.rels_with_props(node, Direction.Outgoing, "transfer", ["ts", "amt"])
    for nbr, ts, amt in zip(nbrs, tss, amts):
        if ts <= last_ts or amt < min_amount:
            continue  # strictly increasing time + amount threshold
        f0 = ts if first_ts is None else first_ts
        if ts - f0 > window_ms:
            continue
        if nbr == start:
            if len(path) >= 2:
                out.append(list(path))
            continue
        if nbr in on_path:
            continue
        path.append(nbr)
        on_path.add(nbr)
        _cycle_dfs(g, start, nbr, ts, f0, min_amount, window_ms, path, on_path, out)
        path.pop()
        on_path.remove(nbr)


def shortest_transfer_path(g, src, dst, start_ms, end_ms):
    """TCR3 — shortest in-window ``transfer`` path (hop count) from src to dst, or
    -1 if unreachable. Unweighted BFS over in-window transfer rels."""
    if src == dst:
        return 0
    visited = {src}
    queue = deque([(src, 0)])
    while queue:
        node, d = queue.popleft()
        nbrs, (ts,) = g.rels_with_props(node, Direction.Outgoing, "transfer", ["ts"])
        for nbr, t in zip(nbrs, ts):
            if t < start_ms or t > end_ms:
                continue
            if nbr == dst:
                return d + 1
            if nbr not in visited:
                visited.add(nbr)
                queue.append((nbr, d + 1))
    return -1


def cr1(g, account, start_ms, end_ms, truncation_limit, truncation_order_asc):
    """TCR1 — accounts reachable from ``account`` by a <=3-hop, time-descending,
    in-window reverse ``transfer`` trace that are signed in by a *blocked* Medium.
    Returns (otherId, distance, mediumId, "Medium"), sorted (dist, id, mediumId)."""
    results = []
    visited = {account}
    queue = deque([(account, 0, _I64_MAX)])  # (node, depth, last_ts)
    while queue:
        node, depth, last_ts = queue.popleft()
        if depth >= 3:
            continue
        nbrs, (tss,) = g.rels_with_props(node, Direction.Incoming, "transfer", ["ts"])
        rels = [(t, nbr) for nbr, t in zip(nbrs, tss) if start_ms <= t <= end_ms and t < last_ts]
        rels.sort(key=lambda x: x[0], reverse=not truncation_order_asc)
        del rels[truncation_limit:]
        for ts, neighbor in rels:
            if neighbor not in visited:
                visited.add(neighbor)
                dist = depth + 1
                # one result per signIn rel (Rust counts per-rel, not per distinct
                # medium), so use the per-rel accessor, not neighbor_ids.
                media, _ = g.rels_with_props(neighbor, Direction.Incoming, "signIn", [])
                for medium in media:
                    if g.get_property(medium, "blocked"):
                        results.append((neighbor, dist, medium, "Medium"))
                queue.append((neighbor, dist, ts))
    results.sort(key=lambda r: (r[1], r[0], r[2]))
    return results


def cr2(g, person, start_ms, end_ms, truncation_limit, truncation_order_asc):
    """TCR2 — from a Person's owned accounts, reverse-trace in-window ``transfer``
    (<=3 hops); for each upstream account sum the loan amount/balance deposited into
    it. Returns (otherId, sumLoanAmount, sumLoanBalance), (sumAmount desc, id asc)."""
    by_acct = {}
    for owned in g.neighbor_ids(person, Direction.Outgoing, ["own"]):
        visited = {owned}
        queue = deque([(owned, 0, _I64_MAX)])
        while queue:
            node, depth, last_ts = queue.popleft()
            if depth >= 3:
                continue
            nbrs, (tss,) = g.rels_with_props(node, Direction.Incoming, "transfer", ["ts"])
            rels = list(zip(tss, nbrs))
            rels.sort(key=lambda x: x[0], reverse=not truncation_order_asc)
            del rels[truncation_limit:]
            for ts, neighbor in rels:
                if ts < start_ms or ts > end_ms or ts >= last_ts:
                    continue
                if neighbor not in visited:
                    visited.add(neighbor)
                    queue.append((neighbor, depth + 1, ts))
        for acct in visited:
            if acct == owned:
                continue
            loans = set()
            amt = bal = 0.0
            loan_nbrs, (dts,) = g.rels_with_props(acct, Direction.Incoming, "deposit", ["ts"])
            for loan, ts in zip(loan_nbrs, dts):
                if ts < start_ms or ts > end_ms:
                    continue
                if loan not in loans:
                    loans.add(loan)
                    amt += g.get_property(loan, "amount") or 0.0
                    bal += g.get_property(loan, "balance") or 0.0
            if loans:
                e = by_acct.get(acct, (0.0, 0.0))
                by_acct[acct] = (e[0] + amt, e[1] + bal)
    out = [(a, x, y) for a, (x, y) in by_acct.items()]
    out.sort(key=lambda r: (-r[1], r[0]))
    return out


def cr5(g, person, start_ms, end_ms, truncation_limit, truncation_order):
    """TCR5 — forward transfer traces (<=3 hops, strictly increasing in-window ts, no
    cycle) from a person's owned accounts; parallel rels to a neighbor collapse to the
    earliest ts. Returns the path node-sequences, sorted by length descending."""
    desc = truncation_order.lower() == "desc"
    out = []
    for start_account in g.neighbor_ids(person, Direction.Outgoing, ["own"]):
        _cr5_dfs(g, start_account, start_ms, end_ms, _I64_MIN, [start_account],
                 {start_account}, out, 0, truncation_limit, desc)
    uniq = sorted({tuple(p) for p in out})
    uniq.sort(key=lambda p: -len(p))
    return [list(p) for p in uniq]


def _cr5_dfs(g, node, start_ms, end_ms, last_ts, path, visited, out, depth, limit, desc):
    if depth >= 3:
        return
    by_neighbor = {}
    nbrs, (tss,) = g.rels_with_props(node, Direction.Outgoing, "transfer", ["ts"])
    for nbr, ts in zip(nbrs, tss):
        if start_ms <= ts <= end_ms and ts > last_ts and nbr not in visited:
            cur = by_neighbor.get(nbr)
            by_neighbor[nbr] = ts if cur is None else min(cur, ts)
    candidates = list(by_neighbor.items())
    if limit > 0 and len(candidates) > limit:
        candidates.sort(key=lambda x: x[1], reverse=desc)
        candidates = candidates[:limit]
    for neighbor, ts in candidates:
        path.append(neighbor)
        visited.add(neighbor)
        out.append(list(path))
        _cr5_dfs(g, neighbor, start_ms, end_ms, ts, path, visited, out, depth + 1, limit, desc)
        path.pop()
        visited.remove(neighbor)


def cr6(g, dst_card, threshold1, threshold2, start_ms, end_ms, truncation_limit, truncation_order):
    """TCR6 — withdrawal after many-to-one. Sources whose in-window transfer
    (amt > threshold1, before the card's last in-window withdrawal of amt > threshold2)
    feeds dst_card. Returns (srcId, sumInAmount, totalWithdrawn), (sum desc, id asc)."""
    _w, (wts, wamt) = g.rels_with_props(dst_card, Direction.Outgoing, "withdraw", ["ts", "amt"])
    withdraws = [(t, a) for t, a in zip(wts, wamt) if start_ms <= t <= end_ms and a > threshold2]
    if not withdraws:
        return []
    total_withdraw = sum(a for _, a in withdraws)
    last_withdraw = max(t for t, _ in withdraws)
    by_src = {}
    srcs, (its, iamt) = g.rels_with_props(dst_card, Direction.Incoming, "transfer", ["ts", "amt"])
    for src, ts, amt in zip(srcs, its, iamt):
        if start_ms <= ts <= end_ms and amt > threshold1 and ts <= last_withdraw:
            by_src[src] = by_src.get(src, 0.0) + amt
    out = [(s, a, total_withdraw) for s, a in by_src.items()]
    out.sort(key=lambda r: (-r[1], r[0]))
    return out


def cr7(g, account, threshold, start_ms, end_ms, truncation_limit, truncation_order_asc):
    """TCR7 — transfer in/out ratio (distinct sources/destinations + in/out amount
    ratio, 3dp). rel_view + a C-speed sum when no filter/truncation binds."""
    no_window = start_ms == _I64_MIN and end_ms == _I64_MAX

    def collect(direction, is_out):
        v = g.rel_view(account, direction, "transfer", ["ts", "amt"])
        n = len(v)
        nbr = memoryview(v.neighbors)
        amt = memoryview(v.col("amt"))
        if no_window and threshold <= 0 and n <= truncation_limit:
            return len(set(nbr)), sum(amt)  # fast path: no filter, C-speed sum
        ts = memoryview(v.col("ts"))
        rels = [(ts[i], amt[i], nbr[i]) for i in range(n)
                if start_ms <= ts[i] <= end_ms and amt[i] > threshold]
        if len(rels) > truncation_limit:
            rels.sort(key=lambda x: x[0], reverse=not truncation_order_asc)
            del rels[truncation_limit:]
        return len({x[2] for x in rels}), sum(x[1] for x in rels)

    num_src, in_amt = collect(Direction.Incoming, False)
    num_dst, out_amt = collect(Direction.Outgoing, True)
    ratio = round(in_amt / out_amt, 3) if out_amt > 0 else -1.0
    return (num_src, num_dst, ratio)


def cr8(g, loan_id, threshold, start_ms, end_ms, truncation_limit, truncation_order):
    """TCR8 — transfer trace after a loan: from each account the loan deposits to,
    trace transfer/withdraw <=3 hops (following only amt > threshold*upstream-in);
    return (dstId, inflow/loanAmount 3dp, minDistance), (dist desc, ratio desc, id asc)."""
    loan_amount = g.get_property(loan_id, "amount") or 1.0
    desc = truncation_order == "DESC"
    dn, (dts, damt) = g.rels_with_props(loan_id, Direction.Outgoing, "deposit", ["ts", "amt"])
    deposits = [(n, a) for n, t, a in zip(dn, dts, damt) if start_ms <= t <= end_ms]
    results = {}  # node -> [total_inflow, min_dist]
    for start_account, deposit_amt in deposits:
        visited = {start_account}
        queue = deque([(start_account, 1, deposit_amt)])
        while queue:
            node, dist, inflow = queue.popleft()
            cur = results.get(node)
            if cur is None:
                results[node] = [inflow, dist]
            else:
                cur[0] += inflow
                cur[1] = min(cur[1], dist)
            if dist >= 3:
                continue
            _u, (uts, uamt) = g.rels_with_props(node, Direction.Incoming, "transfer", ["ts", "amt"])
            upstream_total = sum(a for t, a in zip(uts, uamt) if start_ms <= t <= end_ms)
            rels = []
            for rel_type in ("transfer", "withdraw"):
                on, (ots, oamt) = g.rels_with_props(node, Direction.Outgoing, rel_type, ["ts", "amt"])
                for n2, t, a in zip(on, ots, oamt):
                    if start_ms <= t <= end_ms:
                        rels.append((a, n2))
            rels.sort(key=lambda x: x[0], reverse=desc)
            del rels[truncation_limit:]
            for amt, neighbor in rels:
                if amt > threshold * upstream_total and neighbor not in visited:
                    visited.add(neighbor)
                    queue.append((neighbor, dist + 1, amt))
    out = [(did, round(tot / loan_amount, 3), dist) for did, (tot, dist) in results.items()]
    out.sort(key=lambda r: (-r[2], -r[1], r[0]))
    return out


def cr9(g, account, threshold, start_ms, end_ms, truncation_limit, truncation_asc):
    """TCR9 — money-laundering ratios: repay/deposit-in, repay/transfer-in,
    transfer-out/transfer-in (3dp; -1 on a zero denominator). rel_view + a C-speed
    sum when no filter/truncation binds."""
    no_window = start_ms == _I64_MIN and end_ms == _I64_MAX

    def sum_amt(direction, rel, amt_filter):
        v = g.rel_view(account, direction, rel, ["ts", "amt"])
        amt = v.col("amt")
        if no_window and not (amt_filter and threshold > 0) and len(v) <= truncation_limit:
            return sum(memoryview(amt))  # fast path: no filter/truncation -> C-speed
        ts = memoryview(v.col("ts"))
        am = memoryview(amt)
        rels = [(ts[i], am[i]) for i in range(len(v))
                if start_ms <= ts[i] <= end_ms and (not amt_filter or am[i] >= threshold)]
        if len(rels) > truncation_limit:
            rels.sort(key=lambda x: x[0], reverse=not truncation_asc)
            del rels[truncation_limit:]
        return sum(a for _, a in rels)

    rel1 = sum_amt(Direction.Outgoing, "repay", False)
    rel2 = sum_amt(Direction.Incoming, "deposit", False)
    rel3 = sum_amt(Direction.Outgoing, "transfer", True)
    rel4 = sum_amt(Direction.Incoming, "transfer", True)
    repay = -1.0 if rel2 == 0.0 else round(rel1 / rel2, 3)
    deposit = -1.0 if rel4 == 0.0 else round(rel1 / rel4, 3)
    transfer = -1.0 if rel4 == 0.0 else round(rel3 / rel4, 3)
    return (repay, deposit, transfer)


def cr10(g, person, start_ms, end_ms):
    """TCR10 — investor similarity: other investors who share invested Companies (in
    window) with ``person``. Returns (otherId, sharedCompanyCount), (count desc, id asc)."""
    cn, (cts,) = g.rels_with_props(person, Direction.Outgoing, "invest", ["ts"])
    companies = {c for c, t in zip(cn, cts) if start_ms <= t <= end_ms}
    shared = {}
    for c in companies:
        for other in g.neighbor_ids(c, Direction.Incoming, ["invest"]):
            if other != person:
                shared[other] = shared.get(other, 0) + 1
    out = list(shared.items())
    out.sort(key=lambda r: (-r[1], r[0]))
    return out


def cr12(g, person_id, start_ms, end_ms, truncation_limit, truncation_order_asc):
    """TCR12 — sums of a person's in-window transfers (via accounts they own) into
    Company-owned accounts. Returns (compAccountId, summedAmount), (sum desc, id asc)."""
    companies = set(g.nodes_with_label("Company"))
    person_accounts = list(g.neighbor_ids(person_id, Direction.Outgoing, ["own"]))
    if len(person_accounts) > truncation_limit:
        person_accounts.sort()
        person_accounts = person_accounts[:truncation_limit]
    amounts = {}
    for pa in person_accounts:
        tn, (tts, tamt) = g.rels_with_props(pa, Direction.Outgoing, "transfer", ["ts", "amt"])
        transfers = [(n, a) for n, t, a in zip(tn, tts, tamt) if start_ms <= t <= end_ms]
        if len(transfers) > truncation_limit:
            transfers.sort(key=lambda x: x[1], reverse=not truncation_order_asc)
            transfers = transfers[:truncation_limit]
        for target, amt in transfers:
            if any(owner in companies
                   for owner in g.neighbor_ids(target, Direction.Incoming, ["own"])):
                amounts[target] = amounts.get(target, 0.0) + amt
    out = list(amounts.items())
    out.sort(key=lambda r: (-r[1], r[0]))
    return out


def guarantee_exposure(g, person):
    """TCR11 — a person's loan exposure: walk the ``guarantee`` chain out from
    ``person``, summing the ``apply`` loan amounts they and their guarantees owe."""
    visited = {person}
    queue = deque([person])
    total = 0.0
    while queue:
        p = queue.popleft()
        total += sum(memoryview(g.rel_view(p, Direction.Outgoing, "apply", ["amt"]).col("amt")))
        for nbr in g.neighbor_ids(p, Direction.Outgoing, ["guarantee"]):
            if nbr not in visited:
                visited.add(nbr)
                queue.append(nbr)
    return total
