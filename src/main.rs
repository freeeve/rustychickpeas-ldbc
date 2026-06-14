//! Load real LDBC SNB BI (SF1) data into rustychickpeas and time queries.
//!
//! Two query families run here:
//!   * **Faithful** BI queries (`Q1`, `Q2`, …) — translations of the official
//!     `ldbc/ldbc_snb_bi` Cypher queries, with their real filters, date
//!     parameters and aggregations. These read stored node properties through
//!     the public graph API, so the timings reflect rustychickpeas doing the
//!     actual analytical work (no query optimizer — each is hand-coded).
//!   * **Simplified** patterns (`BI1`–`BI6`) — the lighter namesakes the core
//!     repo's synthetic `ldbc_snb_bi` benchmark uses, kept for continuity with
//!     the synthetic-vs-real comparison.
//!
//! The dataset is the pipe-delimited, gzip-compressed CSV release from
//! <https://datasets.ldbcouncil.org/>. Node IDs there are i64 and only unique
//! within a node type (a Person and a Tag can share id 332), so we keep one
//! i64 -> u32 map per type and remap every relationship endpoint.

use flate2::read::GzDecoder;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use csv::ReaderBuilder;
use rustychickpeas_core::{GraphBuilder, GraphSnapshot, ValueId};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// One Q1 output group: (year, isComment, lengthCategory, messageCount, sumLength).
type Q1Row = (i64, bool, u8, u64, i64);

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's
/// algorithm). Used so date-range filters and N-day window arithmetic are plain
/// integer comparisons.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse an LDBC creationDate ("2010-02-24T08:06:02.996+00:00") into
/// (year, days-since-epoch).
fn parse_date(s: &str) -> Option<(i64, i64)> {
    if s.len() < 10 {
        return None;
    }
    let y: i64 = s[0..4].parse().ok()?;
    let m: i64 = s[5..7].parse().ok()?;
    let d: i64 = s[8..10].parse().ok()?;
    Some((y, days_from_civil(y, m, d)))
}

fn pi64(g: &GraphSnapshot, n: u32, k: &str) -> i64 {
    match g.prop(n, k) {
        Some(ValueId::I64(v)) => v,
        _ => 0,
    }
}

fn pbool(g: &GraphSnapshot, n: u32, k: &str) -> bool {
    matches!(g.prop(n, k), Some(ValueId::Bool(true)))
}

fn pstr<'a>(g: &'a GraphSnapshot, n: u32, k: &str) -> Option<&'a str> {
    match g.prop(n, k) {
        Some(ValueId::Str(s)) => g.resolve_string(s),
        _ => None,
    }
}

/// Call `f` with the requested columns (in order) for every row across all
/// `part-*.csv.gz` files in `dir`. Returns the number of rows visited.
fn for_each_row(dir: &Path, cols: &[&str], mut f: impl FnMut(&[&str])) -> Result<u64> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir {}: {}", dir.display(), e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("gz"))
        .collect();
    files.sort();

    let mut count = 0u64;
    for path in files {
        let decoder = GzDecoder::new(BufReader::new(File::open(&path)?));
        let mut reader = ReaderBuilder::new()
            .delimiter(b'|')
            .has_headers(true)
            .from_reader(decoder);
        let headers = reader.headers()?.clone();
        let idx: Vec<usize> = cols
            .iter()
            .map(|c| {
                headers
                    .iter()
                    .position(|h| h == *c)
                    .ok_or_else(|| format!("column '{}' not in {:?}", c, headers))
            })
            .collect::<std::result::Result<_, _>>()?;

        let mut record = csv::StringRecord::new();
        while reader.read_record(&mut record)? {
            let vals: Vec<&str> = idx.iter().map(|&i| record.get(i).unwrap_or("")).collect();
            f(&vals);
            count += 1;
        }
    }
    Ok(count)
}

#[derive(Default)]
struct Stats {
    persons: u64,
    posts: u64,
    comments: u64,
    tags: u64,
    tag_classes: u64,
    edges: u64,
}

/// Store the message properties the faithful queries read: year, day-number,
/// length, and whether content is present (image-only posts have empty content).
fn set_message_props(b: &mut GraphBuilder, id: u32, creation: &str, content: &str, length: &str) {
    if let Some((year, day)) = parse_date(creation) {
        b.set_prop_i64(id, "year", year).unwrap();
        b.set_prop_i64(id, "day", day).unwrap();
    }
    b.set_prop_i64(id, "len", length.parse::<i64>().unwrap_or(0))
        .unwrap();
    b.set_prop_bool(id, "content", !content.is_empty()).unwrap();
}

/// Load the BI-relevant subgraph from an `initial_snapshot` directory.
fn load_graph(snapshot: &Path) -> Result<(GraphSnapshot, Stats)> {
    let dynamic = snapshot.join("dynamic");
    let static_ = snapshot.join("static");

    let mut builder = GraphBuilder::new(Some(3_000_000), Some(16_000_000));
    let mut next: u32 = 0;
    let mut person: HashMap<i64, u32> = HashMap::new();
    let mut tag: HashMap<i64, u32> = HashMap::new();
    let mut tagclass: HashMap<i64, u32> = HashMap::new();
    let mut post: HashMap<i64, u32> = HashMap::new();
    let mut comment: HashMap<i64, u32> = HashMap::new();
    let mut stats = Stats::default();

    // Static TagClass (load before Tag so HAS_TYPE edges resolve).
    for_each_row(&static_.join("TagClass"), &["id", "name"], |v| {
        if let Ok(lid) = v[0].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &["TagClass"]).unwrap();
            builder.set_prop_str(id, "name", v[1]).unwrap();
            tagclass.insert(lid, id);
        }
    })?;
    stats.tag_classes = tagclass.len() as u64;

    // Static Tag + Tag -[hasType]-> TagClass.
    for_each_row(
        &static_.join("Tag"),
        &["id", "name", "TypeTagClassId"],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Tag"]).unwrap();
                builder.set_prop_str(id, "name", v[1]).unwrap();
                tag.insert(lid, id);
                if let Some(&class) = v[2].parse::<i64>().ok().and_then(|c| tagclass.get(&c)) {
                    builder.add_rel(id, class, "hasType").unwrap();
                    stats.edges += 1;
                }
            }
        },
    )?;
    stats.tags = tag.len() as u64;

    // Persons (before Posts/Comments so hasCreator edges can resolve).
    for_each_row(&dynamic.join("Person"), &["id"], |v| {
        if let Ok(lid) = v[0].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &["Person"]).unwrap();
            person.insert(lid, id);
        }
    })?;
    stats.persons = person.len() as u64;

    // Posts: node + properties + hasCreator (Person -> Post).
    for_each_row(
        &dynamic.join("Post"),
        &["id", "CreatorPersonId", "creationDate", "content", "length"],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Post"]).unwrap();
                set_message_props(&mut builder, id, v[2], v[3], v[4]);
                post.insert(lid, id);
                if let Some(&creator) = v[1].parse::<i64>().ok().and_then(|c| person.get(&c)) {
                    builder.add_rel(creator, id, "hasCreator").unwrap();
                    stats.edges += 1;
                }
            }
        },
    )?;
    stats.posts = post.len() as u64;

    // Comments: node + properties + hasCreator (Person -> Comment).
    for_each_row(
        &dynamic.join("Comment"),
        &["id", "CreatorPersonId", "creationDate", "content", "length"],
        |v| {
            if let Ok(lid) = v[0].parse::<i64>() {
                let id = next;
                next += 1;
                builder.add_node(Some(id), &["Comment"]).unwrap();
                set_message_props(&mut builder, id, v[2], v[3], v[4]);
                comment.insert(lid, id);
                if let Some(&creator) = v[1].parse::<i64>().ok().and_then(|c| person.get(&c)) {
                    builder.add_rel(creator, id, "hasCreator").unwrap();
                    stats.edges += 1;
                }
            }
        },
    )?;
    stats.comments = comment.len() as u64;

    // Post -> Tag.
    for_each_row(
        &dynamic.join("Post_hasTag_Tag"),
        &["PostId", "TagId"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| post.get(&i));
            let t = v[1].parse::<i64>().ok().and_then(|i| tag.get(&i));
            if let (Some(&p), Some(&t)) = (p, t) {
                builder.add_rel(p, t, "hasTag").unwrap();
                stats.edges += 1;
            }
        },
    )?;

    // Comment -> Tag.
    for_each_row(
        &dynamic.join("Comment_hasTag_Tag"),
        &["CommentId", "TagId"],
        |v| {
            let c = v[0].parse::<i64>().ok().and_then(|i| comment.get(&i));
            let t = v[1].parse::<i64>().ok().and_then(|i| tag.get(&i));
            if let (Some(&c), Some(&t)) = (c, t) {
                builder.add_rel(c, t, "hasTag").unwrap();
                stats.edges += 1;
            }
        },
    )?;

    // Person -> Tag interests.
    for_each_row(
        &dynamic.join("Person_hasInterest_Tag"),
        &["personId", "interestId"],
        |v| {
            let p = v[0].parse::<i64>().ok().and_then(|i| person.get(&i));
            let t = v[1].parse::<i64>().ok().and_then(|i| tag.get(&i));
            if let (Some(&p), Some(&t)) = (p, t) {
                builder.add_rel(p, t, "hasInterest").unwrap();
                stats.edges += 1;
            }
        },
    )?;

    Ok((builder.finalize(None), stats))
}

// ============ Faithful LDBC SNB BI queries ============

/// Q1 — Posting summary. Messages before `cutoff_day` (with content), grouped by
/// (year, isComment, length-category); reports counts, average/sum length and
/// share of all messages before the cutoff.
/// Returns (group rows, total-message-count). Cypher: bi-1.cypher.
fn q1_posting_summary(g: &GraphSnapshot, cutoff_day: i64) -> (Vec<Q1Row>, u64) {
    let mut total = 0u64;
    let mut groups: HashMap<(i64, bool, u8), (u64, i64)> = HashMap::new();
    for (label, is_comment) in [("Post", false), ("Comment", true)] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                if pi64(g, msg, "day") >= cutoff_day {
                    continue;
                }
                total += 1;
                if !pbool(g, msg, "content") {
                    continue;
                }
                let len = pi64(g, msg, "len");
                let cat = if len < 40 {
                    0
                } else if len < 80 {
                    1
                } else if len < 160 {
                    2
                } else {
                    3
                };
                let e = groups
                    .entry((pi64(g, msg, "year"), is_comment, cat))
                    .or_insert((0, 0));
                e.0 += 1;
                e.1 += len;
            }
        }
    }
    let mut rows: Vec<Q1Row> = groups
        .into_iter()
        .map(|((y, c, cat), (n, sum))| (y, c, cat, n, sum))
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    (rows, total)
}

/// Q2 — Tag evolution. For tags of a given TagClass, count messages tagged with
/// them in two consecutive 100-day windows from `date0_day`; report the counts
/// and their absolute difference. Cypher: bi-2.cypher.
fn q2_tag_evolution(
    g: &GraphSnapshot,
    date0_day: i64,
    class_name: &str,
) -> Vec<(String, u64, u64, u64)> {
    // Target TagClass node.
    let target = g.nodes_with_label("TagClass").and_then(|tcs| {
        tcs.iter()
            .find(|&tc| pstr(g, tc, "name") == Some(class_name))
    });
    let Some(target) = target else {
        return Vec::new();
    };

    // Tags of that class.
    let mut qualifying: HashSet<u32> = HashSet::new();
    if let Some(tags) = g.nodes_with_label("Tag") {
        for t in tags.iter() {
            if g.out_neighbors_by_type(t, &["hasType"]).contains(&target) {
                qualifying.insert(t);
            }
        }
    }

    let (w1_lo, w1_hi) = (date0_day, date0_day + 100);
    let (w2_lo, w2_hi) = (date0_day + 100, date0_day + 200);
    let mut c1: HashMap<u32, u64> = HashMap::new();
    let mut c2: HashMap<u32, u64> = HashMap::new();
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                let day = pi64(g, msg, "day");
                let in1 = w1_lo <= day && day < w1_hi;
                let in2 = w2_lo <= day && day < w2_hi;
                if !in1 && !in2 {
                    continue;
                }
                for t in g.out_neighbors_by_type(msg, &["hasTag"]) {
                    if qualifying.contains(&t) {
                        *(if in1 { &mut c1 } else { &mut c2 }).entry(t).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    let mut rows: Vec<(String, u64, u64, u64)> = qualifying
        .iter()
        .map(|&t| {
            let n1 = c1.get(&t).copied().unwrap_or(0);
            let n2 = c2.get(&t).copied().unwrap_or(0);
            let name = pstr(g, t, "name").unwrap_or("").to_string();
            (name, n1, n2, n1.abs_diff(n2))
        })
        .collect();
    rows.sort_by(|a, b| b.3.cmp(&a.3).then(a.0.cmp(&b.0)));
    rows.truncate(100);
    rows
}

// ============ Simplified analytical patterns (synthetic-benchmark parity) ============

fn bi1_tag_evolution(g: &GraphSnapshot) -> usize {
    let mut pairs: HashMap<(u32, u32), u32> = HashMap::new();
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                let tags = g.out_neighbors_by_type(msg, &["hasTag"]);
                for i in 0..tags.len() {
                    for j in (i + 1)..tags.len() {
                        let pair = if tags[i] < tags[j] {
                            (tags[i], tags[j])
                        } else {
                            (tags[j], tags[i])
                        };
                        *pairs.entry(pair).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    pairs.len()
}

fn bi3_popular_topics(g: &GraphSnapshot) -> usize {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for label in ["Post", "Comment"] {
        if let Some(nodes) = g.nodes_with_label(label) {
            for msg in nodes.iter() {
                for t in g.out_neighbors_by_type(msg, &["hasTag"]) {
                    *counts.entry(t).or_insert(0) += 1;
                }
            }
        }
    }
    counts.len()
}

fn top_creators(g: &GraphSnapshot, label: &str) -> usize {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    let persons = g.nodes_with_label("Person");
    if let Some(nodes) = g.nodes_with_label(label) {
        for msg in nodes.iter() {
            for &creator in g.in_neighbors(msg) {
                if persons.is_some_and(|p| p.contains(creator)) {
                    *counts.entry(creator).or_insert(0) += 1;
                }
            }
        }
    }
    counts.len()
}

/// Median wall-clock over `runs` timed iterations (after one warmup).
fn time_query(name: &str, runs: usize, mut q: impl FnMut() -> usize) {
    let warm = q();
    let mut samples: Vec<u128> = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        let _ = q();
        samples.push(t.elapsed().as_micros());
    }
    samples.sort_unstable();
    let median_ms = samples[samples.len() / 2] as f64 / 1000.0;
    println!("{name:<34} {median_ms:>9.2} ms   (result={warm})");
}

fn main() -> Result<()> {
    let default = PathBuf::from(
        "data/bi-sf1-composite-merged-fk/graphs/csv/bi/composite-merged-fk/initial_snapshot",
    );
    let snapshot = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(default);
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
    let load_secs = t.elapsed().as_secs_f64();

    let nodes = s.persons + s.posts + s.comments + s.tags + s.tag_classes;
    println!("\n=== LDBC SNB BI — SF1 (real data) ===");
    println!(
        "Loaded {nodes} nodes ({} persons, {} posts, {} comments, {} tags, {} tagclasses)",
        s.persons, s.posts, s.comments, s.tags, s.tag_classes
    );
    println!("       {} edges in {load_secs:.1}s\n", s.edges);

    // --- Faithful LDBC BI queries (official Cypher params) ---
    println!("Faithful LDBC BI queries:");
    let q1_cutoff = days_from_civil(2011, 12, 1);
    let (q1_rows, q1_total) = q1_posting_summary(&graph, q1_cutoff);
    println!(
        "  Q1 posting summary: {} groups over {} messages before 2011-12-01",
        q1_rows.len(),
        q1_total
    );
    for (y, is_c, cat, n, sum) in q1_rows.iter().take(4) {
        let avg = *sum as f64 / *n as f64;
        let kind = if *is_c { "Comment" } else { "Post" };
        println!("     {y} {kind:<7} lenCat={cat}  count={n}  avgLen={avg:.1}");
    }
    let q2_date = days_from_civil(2012, 6, 1);
    let q2_rows = q2_tag_evolution(&graph, q2_date, "MusicalArtist");
    println!(
        "  Q2 tag evolution (MusicalArtist, 2012-06-01): {} tags",
        q2_rows.len()
    );
    for (name, n1, n2, diff) in q2_rows.iter().take(3) {
        println!("     {name:<30} w1={n1} w2={n2} diff={diff}");
    }
    println!();

    let runs = 5;
    println!("Timings (median of {runs}):");
    time_query("Q1 posting summary", runs, || {
        q1_posting_summary(&graph, q1_cutoff).0.len()
    });
    time_query("Q2 tag evolution", runs, || {
        q2_tag_evolution(&graph, q2_date, "MusicalArtist").len()
    });
    // Simplified patterns (parity with the synthetic benchmark).
    time_query("BI1 tag co-evolution (simpl.)", runs, || {
        bi1_tag_evolution(&graph)
    });
    time_query("BI3 popular topics (simpl.)", runs, || {
        bi3_popular_topics(&graph)
    });
    time_query("BI4 top commenters (simpl.)", runs, || {
        top_creators(&graph, "Comment")
    });
    time_query("BI5 active users (simpl.)", runs, || {
        top_creators(&graph, "Post")
    });

    Ok(())
}
