//! Load real LDBC SNB BI (SF1) data into rustychickpeas and time the BI-style
//! analytical queries that the `ldbc_snb_bi` benchmark in the core repo runs
//! against a synthetic graph. The point is an apples-to-apples-as-possible
//! reading of those queries on the real dataset.
//!
//! The dataset is the pipe-delimited, gzip-compressed CSV release from
//! <https://datasets.ldbcouncil.org/>. Node IDs in that data are i64 and are
//! only unique within a node type (a Person and a Tag can share id 332), so we
//! keep one i64 -> u32 map per type and remap every relationship endpoint.
//! Only the labels and relationships the queries traverse are loaded
//! (Person/Post/Comment/Tag + hasCreator/hasTag/hasInterest); properties are
//! skipped since the queries do not read them.

use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

use csv::ReaderBuilder;
use rustychickpeas_core::{GraphBuilder, GraphSnapshot};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

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
    has_creator: u64,
    has_tag: u64,
    has_interest: u64,
}

/// Load the BI-relevant subgraph from an `initial_snapshot` directory.
fn load_graph(snapshot: &Path) -> Result<(GraphSnapshot, Stats)> {
    let dynamic = snapshot.join("dynamic");
    let static_ = snapshot.join("static");

    let mut builder = GraphBuilder::new(Some(3_000_000), Some(15_000_000));
    let mut next: u32 = 0;
    let mut person: HashMap<i64, u32> = HashMap::new();
    let mut tag: HashMap<i64, u32> = HashMap::new();
    let mut post: HashMap<i64, u32> = HashMap::new();
    let mut comment: HashMap<i64, u32> = HashMap::new();
    let mut stats = Stats::default();

    // Static tags.
    for_each_row(&static_.join("Tag"), &["id"], |v| {
        if let Ok(lid) = v[0].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &["Tag"]).unwrap();
            tag.insert(lid, id);
        }
    })?;
    stats.tags = tag.len() as u64;

    // Persons (must precede Posts/Comments so hasCreator edges can resolve).
    for_each_row(&dynamic.join("Person"), &["id"], |v| {
        if let Ok(lid) = v[0].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &["Person"]).unwrap();
            person.insert(lid, id);
        }
    })?;
    stats.persons = person.len() as u64;

    // Posts + hasCreator (Person -> Post).
    for_each_row(&dynamic.join("Post"), &["id", "CreatorPersonId"], |v| {
        if let Ok(lid) = v[0].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &["Post"]).unwrap();
            post.insert(lid, id);
            if let Some(&creator) = v[1].parse::<i64>().ok().and_then(|c| person.get(&c)) {
                builder.add_rel(creator, id, "hasCreator").unwrap();
                stats.has_creator += 1;
            }
        }
    })?;
    stats.posts = post.len() as u64;

    // Comments + hasCreator (Person -> Comment).
    for_each_row(&dynamic.join("Comment"), &["id", "CreatorPersonId"], |v| {
        if let Ok(lid) = v[0].parse::<i64>() {
            let id = next;
            next += 1;
            builder.add_node(Some(id), &["Comment"]).unwrap();
            comment.insert(lid, id);
            if let Some(&creator) = v[1].parse::<i64>().ok().and_then(|c| person.get(&c)) {
                builder.add_rel(creator, id, "hasCreator").unwrap();
                stats.has_creator += 1;
            }
        }
    })?;
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
                stats.has_tag += 1;
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
                stats.has_tag += 1;
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
                stats.has_interest += 1;
            }
        },
    )?;

    Ok((builder.finalize(None), stats))
}

// ---- Queries (mirrors of the synthetic ldbc_snb_bi benchmark) ----

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

fn bi2_tag_person_path(g: &GraphSnapshot) -> usize {
    let persons: Vec<u32> = g
        .nodes_with_label("Person")
        .map(|s| s.iter().collect())
        .unwrap_or_default();
    let mut paths = 0;
    let cap = persons.len().min(100);
    for i in 0..cap {
        let ti = g.out_neighbors_by_type(persons[i], &["hasInterest"]);
        for &pj in &persons[i + 1..cap] {
            let tj = g.out_neighbors_by_type(pj, &["hasInterest"]);
            if ti.iter().any(|t| tj.contains(t)) {
                paths += 1;
            }
        }
    }
    paths
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
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.1));
    v.into_iter().take(10).count()
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
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.1));
    v.into_iter().take(10).count()
}

fn bi6_tag_cooccurrence(g: &GraphSnapshot) -> usize {
    let mut co: HashMap<(u32, u32), u32> = HashMap::new();
    if let Some(nodes) = g.nodes_with_label("Post") {
        for msg in nodes.iter() {
            let tags = g.out_neighbors_by_type(msg, &["hasTag"]);
            for i in 0..tags.len() {
                for j in (i + 1)..tags.len() {
                    let pair = if tags[i] < tags[j] {
                        (tags[i], tags[j])
                    } else {
                        (tags[j], tags[i])
                    };
                    *co.entry(pair).or_insert(0) += 1;
                }
            }
        }
    }
    co.len()
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
    println!("{name:<28} {median_ms:>9.2} ms   (result={warm})");
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

    let nodes = s.persons + s.posts + s.comments + s.tags;
    let edges = s.has_creator + s.has_tag + s.has_interest;
    println!("\n=== LDBC SNB BI — SF1 (real data) ===");
    println!(
        "Loaded {nodes} nodes ({} persons, {} posts, {} comments, {} tags)",
        s.persons, s.posts, s.comments, s.tags
    );
    println!(
        "       {edges} edges ({} hasCreator, {} hasTag, {} hasInterest) in {load_secs:.1}s\n",
        s.has_creator, s.has_tag, s.has_interest
    );

    let runs = 5;
    time_query("BI1 tag co-evolution", runs, || bi1_tag_evolution(&graph));
    time_query("BI2 tag person path", runs, || bi2_tag_person_path(&graph));
    time_query("BI3 popular topics", runs, || bi3_popular_topics(&graph));
    time_query("BI4 top commenters", runs, || {
        top_creators(&graph, "Comment")
    });
    time_query("BI5 active users", runs, || top_creators(&graph, "Post"));
    time_query("BI6 tag co-occurrence", runs, || {
        bi6_tag_cooccurrence(&graph)
    });

    Ok(())
}
