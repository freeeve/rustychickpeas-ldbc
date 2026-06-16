//! SNB Interactive (IC) workload: seed-anchored short/complex reads over the
//! same `initial_snapshot` the BI family loads (no extra download). Each query
//! is a hand-coded traversal reusing the loaded schema; the feasible tier
//! (IC1/2/9/13/14 + short reads IS1/2/3/5) is implemented here, the rest are
//! deferred (they need Forum-membership / tag-co-occurrence / organisation
//! expansions noted in tasks/003).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::harness::{emit_json, jstr, time_query, Result};
use crate::loader::load_graph;
use crate::props::*;

/// Reproducible IC substitution parameters derived from the loaded graph.
pub struct IcSeeds {
    /// Well-connected start person (max `knows` degree).
    pub person: u32,
    /// Upper creation-day bound that contains messages (for the "recent" reads).
    pub max_day: i64,
    /// A first name present among `person`'s <=3-hop friends (so IC1 is non-empty).
    pub first_name: String,
    /// A person reachable from `person` over `knows` (for IC13/IC14).
    pub person_b: u32,
}

/// Pick deterministic, reproducible IC seeds from the graph: the highest-degree
/// person (ties broken by smallest id), a fixed late date window, a first name
/// common among that person's neighbourhood, and the farthest person reachable
/// over `knows` (ties broken by smallest id).
pub fn pick_seeds(g: &GraphSnapshot) -> Option<IcSeeds> {
    let persons = g.nodes_with_label("Person")?;
    let mut best: Option<(u32, u32)> = None; // (degree, person)
    for p in persons.iter() {
        let deg = g.neighbors_by_type(p, Direction::Outgoing, "knows").count() as u32;
        best = Some(match best {
            Some((bd, bp)) if bd > deg || (bd == deg && bp <= p) => (bd, bp),
            _ => (deg, p),
        });
    }
    let person = best?.1;

    // A first name common in the start person's <=3-hop neighbourhood.
    let near = g.bfs_distances(person, Direction::Outgoing, "knows", Some(3));
    let mut name_counts: HashMap<String, u32> = HashMap::new();
    for (&p, &d) in &near {
        if d == 0 {
            continue;
        }
        if let Some(fname) = pstr(g, p, "fname") {
            *name_counts.entry(fname.to_string()).or_insert(0) += 1;
        }
    }
    let first_name = name_counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(name, _)| name)?;

    // Farthest reachable person over `knows`, smallest id on ties.
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", None);
    let person_b = reach
        .iter()
        .filter(|(_, &d)| d >= 1)
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(&p, _)| p)?;

    Some(IcSeeds {
        person,
        max_day: days_from_civil(2013, 1, 1),
        first_name,
        person_b,
    })
}

/// IC1 — friends within 3 `knows` hops whose first name matches, ordered by
/// (distance, lastName, id). Returns (friend, distance, lastName), top 20.
pub fn ic1_friends_by_name(g: &GraphSnapshot, person: u32, first_name: &str) -> Vec<(u32, u32, String)> {
    let dist = g.bfs_distances(person, Direction::Outgoing, "knows", Some(3));
    let mut rows: Vec<(u32, u32, String)> = dist
        .iter()
        .filter(|(&p, &d)| d >= 1 && pstr(g, p, "fname") == Some(first_name))
        .map(|(&p, &d)| (p, d, pstr(g, p, "lname").unwrap_or("").to_string()))
        .collect();
    rows.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(a.2.cmp(&b.2))
            .then(pi64(g, a.0, "plid").cmp(&pi64(g, b.0, "plid")))
    });
    rows.truncate(20);
    rows
}

/// IC2 — the 20 most recent messages by the seed's friends, created on/before
/// `max_day`, ordered by (creationDate desc, id). Returns (message, ms).
pub fn ic2_recent_messages(g: &GraphSnapshot, person: u32, max_day: i64) -> Vec<(u32, i64)> {
    let mut rows: Vec<(u32, i64)> = Vec::new();
    for friend in g.neighbors_by_type(person, Direction::Outgoing, "knows") {
        for msg in g.neighbors_by_type(friend, Direction::Outgoing, "hasCreator") {
            if pi64(g, msg, "day") <= max_day {
                rows.push((msg, pi64(g, msg, "ms")));
            }
        }
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(20);
    rows
}

/// IC9 — the 20 most recent messages by the seed's friends and friends-of-friends
/// (<=2 `knows` hops, excluding self), created on/before `max_day`.
pub fn ic9_fof_messages(g: &GraphSnapshot, person: u32, max_day: i64) -> Vec<(u32, i64)> {
    let reach = g.bfs_distances(person, Direction::Outgoing, "knows", Some(2));
    let mut rows: Vec<(u32, i64)> = Vec::new();
    for (&p, &d) in &reach {
        if d == 0 {
            continue;
        }
        for msg in g.neighbors_by_type(p, Direction::Outgoing, "hasCreator") {
            if pi64(g, msg, "day") <= max_day {
                rows.push((msg, pi64(g, msg, "ms")));
            }
        }
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(20);
    rows
}

/// IC13 — unweighted shortest-path length between two persons in the `knows`
/// graph (`-1` if unreachable). Built on the core `bfs_distances` primitive.
pub fn ic13_shortest_path(g: &GraphSnapshot, p1: u32, p2: u32) -> i64 {
    if p1 == p2 {
        return 0;
    }
    g.bfs_distances(p1, Direction::Outgoing, "knows", None)
        .get(&p2)
        .map(|&d| d as i64)
        .unwrap_or(-1)
}

/// IC14 — a weighted shortest path between two persons, where each `knows` edge
/// costs `1 / (interactions + 1)` (more reply interactions = cheaper). Returns
/// the path and its total cost, or `None` if unreachable.
pub fn ic14_weighted_path(
    g: &GraphSnapshot,
    p1: u32,
    p2: u32,
    interaction: &HashMap<(u32, u32), u32>,
) -> Option<(Vec<u32>, f64)> {
    let sp = g.dijkstra(p1, Direction::Outgoing, "knows", Some(p2), |from, rel| {
        let key = (from.min(rel.neighbor), from.max(rel.neighbor));
        let w = interaction.get(&key).copied().unwrap_or(0);
        1.0 / (w as f64 + 1.0)
    });
    Some((sp.path_to(p2)?, sp.distance(p2)?))
}

/// Reply interactions between message creators, keyed by the unordered person
/// pair — the IC14 edge-weight source (mirrors the BI Q19 projection).
pub fn build_knows_interaction(g: &GraphSnapshot) -> HashMap<(u32, u32), u32> {
    let mut m: HashMap<(u32, u32), u32> = HashMap::new();
    if let Some(comments) = g.nodes_with_label("Comment") {
        for c in comments.iter() {
            let Some(a) = g.neighbors_by_type(c, Direction::Incoming, "hasCreator").next() else {
                continue;
            };
            for parent in g.neighbors_by_type(c, Direction::Outgoing, "replyOf") {
                if let Some(b) = g.neighbors_by_type(parent, Direction::Incoming, "hasCreator").next() {
                    if a != b {
                        *m.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    m
}

/// IS1 — a person's profile: (firstName, lastName, creation day).
pub fn is1_profile(g: &GraphSnapshot, person: u32) -> Option<(String, String, i64)> {
    Some((
        pstr(g, person, "fname")?.to_string(),
        pstr(g, person, "lname")?.to_string(),
        pi64(g, person, "pday"),
    ))
}

/// IS2 — a person's own 10 most recent messages on/before `max_day`.
pub fn is2_recent_of_person(g: &GraphSnapshot, person: u32, max_day: i64) -> Vec<(u32, i64)> {
    let mut rows: Vec<(u32, i64)> = g
        .neighbors_by_type(person, Direction::Outgoing, "hasCreator")
        .filter(|&m| pi64(g, m, "day") <= max_day)
        .map(|m| (m, pi64(g, m, "ms")))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(10);
    rows
}

/// IS3 — a person's direct `knows` friends (sorted by id).
pub fn is3_friends(g: &GraphSnapshot, person: u32) -> Vec<u32> {
    let mut f: Vec<u32> = g
        .neighbors_by_type(person, Direction::Outgoing, "knows")
        .collect();
    f.sort_unstable();
    f
}

/// IS5 — the creator of a message.
pub fn is5_message_creator(g: &GraphSnapshot, message: u32) -> Option<u32> {
    g.neighbors_by_type(message, Direction::Incoming, "hasCreator")
        .next()
}

/// Load the snapshot, pick seeds, smoke-check each feasible IC query, then time
/// them (median of 5).
pub fn run() -> Result<()> {
    let default = PathBuf::from(
        "data/bi-sf1-composite-merged-fk/graphs/csv/bi/composite-merged-fk/initial_snapshot",
    );
    let snapshot = std::env::args().nth(1).map(PathBuf::from).unwrap_or(default);
    if !snapshot.join("dynamic").is_dir() {
        return Err(format!(
            "no 'dynamic' dir under {}; pass the initial_snapshot path as arg 1",
            snapshot.display()
        )
        .into());
    }

    eprintln!("Loading LDBC graph from {} ...", snapshot.display());
    let t = Instant::now();
    let (graph, s) = load_graph(&snapshot)?;
    println!("\n=== LDBC SNB Interactive — SF1 (real data) ===");
    println!(
        "Loaded {} persons, {} posts, {} comments in {:.1}s\n",
        s.persons,
        s.posts,
        s.comments,
        t.elapsed().as_secs_f64()
    );

    let seeds = pick_seeds(&graph).ok_or("could not pick IC seeds (no persons?)")?;
    println!(
        "Seeds: person={} (plid {}), person_b={} (plid {}), firstName=\"{}\", maxDay={}",
        seeds.person,
        pi64(&graph, seeds.person, "plid"),
        seeds.person_b,
        pi64(&graph, seeds.person_b, "plid"),
        seeds.first_name,
        seeds.max_day
    );

    // Cross-check emit: dump comparable projections (LDBC ids / ms timestamps,
    // not internal node ids) so `kuzu/run_ic.py` can diff against Kùzu via the
    // shared `compare.py`. Emit mode skips the timing block.
    if let Ok(dir) = std::env::var("LDBC_EMIT_JSON") {
        let plid = |p: u32| pi64(&graph, p, "plid");
        let arr_ms = |rows: &[(u32, i64)]| {
            format!(
                "[{}]",
                rows.iter()
                    .map(|(_, ms)| format!("[{ms}]"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let is2 = is2_recent_of_person(&graph, seeds.person, seeds.max_day);
        let is5 = is2.first().and_then(|&(m, _)| is5_message_creator(&graph, m));
        let mut friends: Vec<i64> = is3_friends(&graph, seeds.person)
            .iter()
            .map(|&f| plid(f))
            .collect();
        friends.sort_unstable();
        emit_json(
            &dir,
            "seeds.json",
            format!(
                "{{\"person\":{},\"person_b\":{},\"first_name\":{},\"max_day\":{}}}",
                plid(seeds.person),
                plid(seeds.person_b),
                jstr(&seeds.first_name),
                seeds.max_day
            ),
        );
        emit_json(
            &dir,
            "ic2.rust.json",
            arr_ms(&ic2_recent_messages(&graph, seeds.person, seeds.max_day)),
        );
        emit_json(
            &dir,
            "ic9.rust.json",
            arr_ms(&ic9_fof_messages(&graph, seeds.person, seeds.max_day)),
        );
        emit_json(
            &dir,
            "ic13.rust.json",
            format!("[[{}]]", ic13_shortest_path(&graph, seeds.person, seeds.person_b)),
        );
        emit_json(&dir, "is2.rust.json", arr_ms(&is2));
        emit_json(
            &dir,
            "is3.rust.json",
            format!(
                "[{}]",
                friends
                    .iter()
                    .map(|p| format!("[{p}]"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
        emit_json(&dir, "is5.rust.json", format!("[[{}]]", is5.map(plid).unwrap_or(-1)));
        println!("emitted IC cross-check JSON (ic2/ic9/ic13, is2/is3/is5) to {dir}");
        return Ok(());
    }

    // Smoke checks (shape, mirroring the BI "surfaces real data" assertions).
    let ic1 = ic1_friends_by_name(&graph, seeds.person, &seeds.first_name);
    let ic2 = ic2_recent_messages(&graph, seeds.person, seeds.max_day);
    let ic9 = ic9_fof_messages(&graph, seeds.person, seeds.max_day);
    let ic13 = ic13_shortest_path(&graph, seeds.person, seeds.person_b);
    let interaction = build_knows_interaction(&graph);
    let ic14 = ic14_weighted_path(&graph, seeds.person, seeds.person_b, &interaction);
    assert!(!ic1.is_empty(), "IC1 returned no friends for the seed name");
    assert!(!ic2.is_empty(), "IC2 returned no recent messages");
    assert!(!ic9.is_empty(), "IC9 returned no FoF messages");
    assert!(ic13 >= 1, "IC13 path length should be >= 1 for a reachable pair");
    assert!(ic14.is_some(), "IC14 found no weighted path");
    println!(
        "  IC1 rows: {}; IC2 recent: {}; IC9 FoF: {}; IC13 hops: {}; IC14 path len: {}",
        ic1.len(),
        ic2.len(),
        ic9.len(),
        ic13,
        ic14.as_ref().map(|(p, _)| p.len()).unwrap_or(0)
    );

    let runs = 5;
    println!("\nTimings (median of {runs}):");
    time_query("IC1 friends-by-name", runs, || {
        ic1_friends_by_name(&graph, seeds.person, &seeds.first_name).len()
    });
    time_query("IC2 recent friend messages", runs, || {
        ic2_recent_messages(&graph, seeds.person, seeds.max_day).len()
    });
    time_query("IC9 recent FoF messages", runs, || {
        ic9_fof_messages(&graph, seeds.person, seeds.max_day).len()
    });
    time_query("IC13 unweighted shortest path", runs, || {
        ic13_shortest_path(&graph, seeds.person, seeds.person_b).max(0) as usize
    });
    time_query("IC14 weighted shortest path", runs, || {
        ic14_weighted_path(&graph, seeds.person, seeds.person_b, &interaction)
            .map(|(p, _)| p.len())
            .unwrap_or(0)
    });
    time_query("IS1 person profile", runs, || {
        is1_profile(&graph, seeds.person).is_some() as usize
    });
    time_query("IS2 person recent messages", runs, || {
        is2_recent_of_person(&graph, seeds.person, seeds.max_day).len()
    });
    time_query("IS3 person friends", runs, || {
        is3_friends(&graph, seeds.person).len()
    });

    println!(
        "\nDeferred (need more schema loaded): IC3-IC8, IC10-IC12 (Forum membership, \
         tag co-occurrence, organisation expansions), IS4/IS6/IS7."
    );
    Ok(())
}
