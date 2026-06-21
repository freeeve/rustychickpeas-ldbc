//! Faithful LDBC SNB BI query family (Q1–Q20) plus the simplified BI1–6
//! patterns, and `run`: load an `initial_snapshot`, then print and time every
//! query.

use std::path::PathBuf;
use std::time::Instant;

use rustychickpeas_core::I64Col;

use crate::harness::{emit_json, jstr, time_query, Result};
use crate::loader::load_graph;
use crate::props::*;

mod faithful_a;
mod faithful_b;
mod faithful_c;

use faithful_a::*;
use faithful_b::*;
use faithful_c::*;

/// One Q1 output group: (year, isComment, lengthCategory, messageCount, sumLength).
pub(crate) type Q1Row = (i64, bool, u8, u64, i64);

/// Read a resolved i64 column at `n` (0 if absent). Takes the [`I64Col`] reader
/// (dense-slice fast path) by value — it is `Copy` and `Sync`, so it stays usable
/// in the parallel scan in Q1.
#[inline]
pub(crate) fn i64_or_zero(c: Option<I64Col>, n: u32) -> i64 {
    c.and_then(|c| c.get(n)).unwrap_or(0)
}

pub fn run() -> Result<()> {
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
    println!("       {} rels in {load_secs:.1}s\n", s.rels);

    // --- Faithful LDBC BI queries (official Cypher params) ---
    // Set LDBC_EMIT_JSON=<dir> to dump Q1/Q2 result rows for the Kùzu
    // cross-check (and skip the slower downstream queries).
    let emit = std::env::var("LDBC_EMIT_JSON").ok();

    println!("Faithful LDBC BI queries:");
    let q1_cutoff = days_from_civil(2011, 12, 1);
    let t_q1 = Instant::now();
    let (q1_rows, q1_total) = q1_posting_summary(&graph, q1_cutoff);
    let q1_ms = t_q1.elapsed().as_secs_f64() * 1000.0;
    println!(
        "  Q1 posting summary: {} groups over {} messages before 2011-12-01  [{q1_ms:.1} ms]",
        q1_rows.len(),
        q1_total
    );
    for (y, is_c, cat, n, sum) in q1_rows.iter().take(4) {
        let avg = *sum as f64 / *n as f64;
        let kind = if *is_c { "Comment" } else { "Post" };
        println!("     {y} {kind:<7} lenCat={cat}  count={n}  avgLen={avg:.1}");
    }
    let q2_date = days_from_civil(2012, 6, 1);
    let t_q2 = Instant::now();
    let q2_rows = q2_tag_evolution(&graph, q2_date, "MusicalArtist");
    let q2_ms = t_q2.elapsed().as_secs_f64() * 1000.0;
    println!(
        "  Q2 tag evolution (MusicalArtist, 2012-06-01): {} tags  [{q2_ms:.1} ms]",
        q2_rows.len()
    );
    for (name, n1, n2, diff) in q2_rows.iter().take(3) {
        println!("     {name:<30} w1={n1} w2={n2} diff={diff}");
    }

    let q7_rows = q7_related_topics(&graph, "Enrique_Iglesias");
    println!(
        "  Q7 related topics (Enrique_Iglesias): {} related tags",
        q7_rows.len()
    );
    for (name, c) in q7_rows.iter().take(3) {
        println!("     {name:<30} comments={c}");
    }
    let q12_min = days_from_civil(2010, 7, 22);
    let q12_rows = q12_message_counts(&graph, q12_min, 20, &["ar", "hu"]);
    println!(
        "  Q12 message counts (len<20, after 2010-07-22, lang ar/hu): {} buckets",
        q12_rows.len()
    );
    for (mc, pc) in q12_rows.iter().take(3) {
        println!("     messageCount={mc} -> persons={pc}");
    }
    let q5_rows = q5_active_posters(&graph, "Abbas_I_of_Persia");
    println!(
        "  Q5 active posters (Abbas_I_of_Persia): {} persons",
        q5_rows.len()
    );
    for (_p, m, r, l, score) in q5_rows.iter().take(3) {
        println!("     msgs={m} replies={r} likes={l} score={score}");
    }
    let q6_rows = q6_authoritative(&graph, "Arnold_Schwarzenegger");
    println!(
        "  Q6 authoritative users (Arnold_Schwarzenegger): {} persons",
        q6_rows.len()
    );
    for (_p, score) in q6_rows.iter().take(3) {
        println!("     authorityScore={score}");
    }

    if let Some(dir) = emit.as_deref() {
        // Canonical column order matches the faithful Kùzu harness's emit.
        let mut s = String::from("["); // Q1: [year, isComment, cat, cnt, sumLen]
        for (i, (y, c, cat, n, sum)) in q1_rows.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "[{y},{},{cat},{n},{sum}]",
                if *c { "true" } else { "false" }
            ));
        }
        s.push(']');
        emit_json(dir, "q1.rust.json", s);

        let mut s = String::from("["); // Q2: [name, w1, w2, diff]
        for (i, (name, n1, n2, diff)) in q2_rows.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{n1},{n2},{diff}]", jstr(name)));
        }
        s.push(']');
        emit_json(dir, "q2.rust.json", s);

        // Map internal person NodeId -> original LDBC id (stored as `plid`) so
        // ids line up with Kùzu's person.id.
        let plid = |n: u32| graph.prop(n, "plid").i64_or(0);
        let mut s = String::from("["); // Q5: [pid, messageCount, replyCount, likeCount, score]
        for (i, (p, m, r, l, score)) in q5_rows.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{m},{r},{l},{score}]", plid(*p)));
        }
        s.push(']');
        emit_json(dir, "q5.rust.json", s);

        let mut s = String::from("["); // Q6: [pid, score]
        for (i, (p, score)) in q6_rows.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{score}]", plid(*p)));
        }
        s.push(']');
        emit_json(dir, "q6.rust.json", s);

        let mut s = String::from("["); // Q7: [name, count]
        for (i, (name, c)) in q7_rows.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{c}]", jstr(name)));
        }
        s.push(']');
        emit_json(dir, "q7.rust.json", s);

        let mut s = String::from("["); // Q12: [messageCount, personCount]
        for (i, (mc, pc)) in q12_rows.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{mc},{pc}]"));
        }
        s.push(']');
        emit_json(dir, "q12.rust.json", s);

        // Q11: single triangle count -> [[count]]
        let q11 = q11_friend_triangles(
            &graph,
            "India",
            days_from_civil(2012, 9, 29),
            days_from_civil(2013, 1, 1),
        );
        emit_json(dir, "q11.rust.json", format!("[[{q11}]]"));

        let q9 = q9_thread_initiators(
            &graph,
            days_from_civil(2011, 10, 1),
            days_from_civil(2011, 10, 15),
        );
        let mut s = String::from("["); // Q9: [pid, threads, messages]
        for (i, (p, t, m)) in q9.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{t},{m}]", plid(*p)));
        }
        s.push(']');
        emit_json(dir, "q9.rust.json", s);

        let q8 = q8_central_person(
            &graph,
            "Che_Guevara",
            days_from_civil(2011, 7, 20),
            days_from_civil(2011, 7, 25),
        );
        let mut s = String::from("["); // Q8: [pid, score, friendsScore]
        for (i, (p, sc, fs)) in q8.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{sc},{fs}]", plid(*p)));
        }
        s.push(']');
        emit_json(dir, "q8.rust.json", s);

        let q13 = q13_zombies(&graph, "France", days_from_civil(2013, 1, 1), 2013 * 12 + 1);
        let mut s = String::from("["); // Q13: [pid, zlc, tlc]
        for (i, (p, zlc, tlc)) in q13.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{zlc},{tlc}]", plid(*p)));
        }
        s.push(']');
        emit_json(dir, "q13.rust.json", s);

        // Q19: weighted SP over interaction graph -> [p1, p2, dist(6dp)]
        let interaction = build_interaction_map(&graph);
        if let (Some(c1), Some(c2)) = (place_by_lid(&graph, 669), place_by_lid(&graph, 648)) {
            let q19 = q19_interaction_path(&graph, c1, c2, &interaction);
            let mut s = String::from("[");
            for (i, (p1, p2, d)) in q19.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("[{},{},{:.6}]", plid(*p1), plid(*p2), d));
            }
            s.push(']');
            emit_json(dir, "q19.rust.json", s);
        }

        // Q20: weighted SP over cohort graph -> [pid, dist(6dp)]
        let studyat = build_studyat(&graph);
        let study_wm = build_study_weight_map(&graph, &studyat);
        if let (Some(co), Some(p2)) = (
            org_by_name(&graph, "Company", "Falcon_Air"),
            person_by_plid(&graph, 66),
        ) {
            let q20 = q20_recruitment(&graph, co, p2, &study_wm);
            let mut s = String::from("[");
            for (i, (p1, d)) in q20.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("[{},{:.6}]", plid(*p1), d));
            }
            s.push(']');
            emit_json(dir, "q20.rust.json", s);
        }

        let q18 = q18_friend_recommendation(&graph, "Frank_Sinatra");
        let mut s = String::from("["); // Q18: [p1, p2, mutualCount]
        for (i, (p1, p2, c)) in q18.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{},{c}]", plid(*p1), plid(*p2)));
        }
        s.push(']');
        emit_json(dir, "q18.rust.json", s);

        let q14 = q14_international_dialog(&graph, "Chile", "Argentina");
        let mut s = String::from("["); // Q14: [p1, p2, cityName, score]
        for (i, (p1, p2, cn, sc)) in q14.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{},{},{sc}]", plid(*p1), plid(*p2), jstr(cn)));
        }
        s.push(']');
        emit_json(dir, "q14.rust.json", s);

        let ra16 = q16_param_result(&graph, "Meryl_Streep", days_from_civil(2012, 9, 16), 4);
        let rb16 = q16_param_result(&graph, "Hank_Williams", days_from_civil(2012, 5, 8), 4);
        // Cross-check the per-param graph work (q16a/q16b) AND the A∩B result (q16);
        // the official params yield an empty intersection at SF1.
        for (name, m) in [("q16a.rust.json", &ra16), ("q16b.rust.json", &rb16)] {
            let mut v: Vec<(i64, i64)> = m.iter().map(|(&p, &c)| (plid(p), c)).collect();
            v.sort();
            let mut s = String::from("[");
            for (i, (p, c)) in v.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("[{p},{c}]"));
            }
            s.push(']');
            emit_json(dir, name, s);
        }
        let q16 = q16_fake_news(&graph, &ra16, &rb16);
        let mut s = String::from("["); // Q16: [pid, cmA, cmB]
        for (i, (p, ca, cb)) in q16.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{ca},{cb}]", plid(*p)));
        }
        s.push(']');
        emit_json(dir, "q16.rust.json", s);

        let q10_person: i64 = std::env::var("LDBC_Q10_PERSON")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3470);
        let q10 = q10_experts(&graph, q10_person, "China", "MusicalArtist", 3, 4);
        let mut s = String::from("["); // Q10: [eid, tagName, messageCount]
        for (i, (e, tn, c)) in q10.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{},{},{c}]", plid(*e), jstr(tn)));
        }
        s.push(']');
        emit_json(dir, "q10.rust.json", s);

        let q3 = q3_popular_topics(&graph, "Burma", "MusicalArtist");
        let mut s = String::from("["); // Q3: [forumId, title, fday, moderatorId, messageCount]
        for (i, (fid, title, fday, pid, c)) in q3.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{fid},{},{fday},{pid},{c}]", jstr(title)));
        }
        s.push(']');
        emit_json(dir, "q3.rust.json", s);

        let (q4, q4_top) = q4_top_creators(&graph, days_from_civil(2010, 1, 29));
        let mut s = String::from("["); // Q4: [pid, messageCount]
        for (i, (p, c)) in q4.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{p},{c}]"));
        }
        s.push(']');
        emit_json(dir, "q4.rust.json", s);
        let mut s = String::from("[");
        for (i, f) in q4_top.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{f}]"));
        }
        s.push(']');
        emit_json(dir, "q4forums.rust.json", s);

        let q15 = q15_weighted_path(
            &graph,
            14,
            16,
            days_from_civil(2010, 11, 1),
            days_from_civil(2010, 12, 1),
        );
        emit_json(dir, "q15.rust.json", format!("[[{:.6}]]", q15));

        let q17 = q17_information_propagation(&graph, "Slavoj_Žižek", 4);
        let mut s = String::from("["); // Q17: [pid, messageCount]
        for (i, (p, c)) in q17.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("[{p},{c}]"));
        }
        s.push(']');
        emit_json(dir, "q17.rust.json", s);

        eprintln!("emitted Q1..Q20 cross-check JSON to {dir}; skipping downstream queries");
        return Ok(());
    }
    let q8_start = days_from_civil(2011, 7, 20);
    let q8_end = days_from_civil(2011, 7, 25);
    let q8_rows = q8_central_person(&graph, "Che_Guevara", q8_start, q8_end);
    println!(
        "  Q8 central person (Che_Guevara, 2011-07-20..25): {} persons",
        q8_rows.len()
    );
    for (_p, s, fs) in q8_rows.iter().take(3) {
        println!("     score={s} friendsScore={fs}");
    }
    let q11_start = days_from_civil(2012, 9, 29);
    let q11_end = days_from_civil(2013, 1, 1);
    let q11_count = q11_friend_triangles(&graph, "India", q11_start, q11_end);
    println!("  Q11 friend triangles (India, 2012-09-29..2013-01-01): {q11_count} triangles");
    let q9_start = days_from_civil(2011, 10, 1);
    let q9_end = days_from_civil(2011, 10, 15);
    let q9_rows = q9_thread_initiators(&graph, q9_start, q9_end);
    println!(
        "  Q9 thread initiators (2011-10-01..15): {} persons",
        q9_rows.len()
    );
    let q13_end = days_from_civil(2013, 1, 1);
    let q13_ym = 2013 * 12 + 1;
    let q13_rows = q13_zombies(&graph, "France", q13_end, q13_ym);
    println!(
        "  Q13 zombies (France, before 2013-01-01): {} zombies",
        q13_rows.len()
    );
    let (reach, ecc) = knows_reachability(&graph);
    println!(
        "  bounded-BFS knows-reachability from person[0]: {reach} reachable, eccentricity {ecc} hops"
    );
    let interaction = build_interaction_map(&graph);
    let q19_cities = place_by_lid(&graph, 669).zip(place_by_lid(&graph, 648));
    match q19_cities {
        Some((c1, c2)) => {
            let q19 = q19_interaction_path(&graph, c1, c2, &interaction);
            println!(
                "  Q19 interaction path (cities 669<->648): {} pairs over {} interaction rels",
                q19.len(),
                interaction.len()
            );
            if let Some((p1, p2, w)) = q19.first() {
                println!("     best: person {p1} -> person {p2}, total weight {w:.4}");
            }
        }
        None => println!("  Q19: city 669 or 648 not present in dataset"),
    }
    let studyat = build_studyat(&graph);
    let study_wm = build_study_weight_map(&graph, &studyat);
    let q20_args = org_by_name(&graph, "Company", "Falcon_Air").zip(person_by_plid(&graph, 66));
    match q20_args {
        Some((co, p2)) => {
            let q20 = q20_recruitment(&graph, co, p2, &study_wm);
            println!(
                "  Q20 recruitment (Falcon_Air -> person 66): {} candidates over {} study rels",
                q20.len(),
                study_wm.len()
            );
            if let Some((p1, w)) = q20.first() {
                println!("     best: person {p1}, total weight {w:.1}");
            }
        }
        None => println!("  Q20: company Falcon_Air or person 66 not present"),
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
    time_query("Q7 related topics", runs, || {
        q7_related_topics(&graph, "Enrique_Iglesias").len()
    });
    time_query("Q12 message counts", runs, || {
        q12_message_counts(&graph, q12_min, 20, &["ar", "hu"]).len()
    });
    time_query("Q5 active posters", runs, || {
        q5_active_posters(&graph, "Abbas_I_of_Persia").len()
    });
    time_query("Q6 authoritative users", runs, || {
        q6_authoritative(&graph, "Arnold_Schwarzenegger").len()
    });
    time_query("Q8 central person", runs, || {
        q8_central_person(&graph, "Che_Guevara", q8_start, q8_end).len()
    });
    time_query("Q11 friend triangles", runs, || {
        q11_friend_triangles(&graph, "India", q11_start, q11_end) as usize
    });
    time_query("Q9 thread initiators", runs, || {
        q9_thread_initiators(&graph, q9_start, q9_end).len()
    });
    time_query("Q13 zombies", runs, || {
        q13_zombies(&graph, "France", q13_end, q13_ym).len()
    });
    time_query("bounded-BFS knows reachability", runs, || {
        knows_reachability(&graph).0
    });
    if let Some((c1, c2)) = q19_cities {
        time_query("Q19 interaction path", runs, || {
            q19_interaction_path(&graph, c1, c2, &interaction).len()
        });
    }
    if let Some((co, p2)) = q20_args {
        time_query("Q20 recruitment", runs, || {
            q20_recruitment(&graph, co, p2, &study_wm).len()
        });
    }
    time_query("Q3 popular topics", runs, || {
        q3_popular_topics(&graph, "Burma", "MusicalArtist").len()
    });
    time_query("Q4 top creators", runs, || {
        q4_top_creators(&graph, days_from_civil(2010, 1, 29))
            .0
            .len()
    });
    let q10_person: i64 = std::env::var("LDBC_Q10_PERSON")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3470);
    time_query("Q10 experts", runs, || {
        q10_experts(&graph, q10_person, "China", "MusicalArtist", 3, 4).len()
    });
    time_query("Q14 international dialog", runs, || {
        q14_international_dialog(&graph, "Chile", "Argentina").len()
    });
    time_query("Q15 weighted path", runs, || {
        let _ = q15_weighted_path(
            &graph,
            14,
            16,
            days_from_civil(2010, 11, 1),
            days_from_civil(2010, 12, 1),
        );
        1
    });
    time_query("Q16 fake news", runs, || {
        let ra = q16_param_result(&graph, "Meryl_Streep", days_from_civil(2012, 9, 16), 4);
        let rb = q16_param_result(&graph, "Hank_Williams", days_from_civil(2012, 5, 8), 4);
        q16_fake_news(&graph, &ra, &rb).len()
    });
    time_query("Q17 information propagation", runs, || {
        q17_information_propagation(&graph, "Slavoj_Žižek", 4).len()
    });
    time_query("Q18 friend recommendation", runs, || {
        q18_friend_recommendation(&graph, "Frank_Sinatra").len()
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
