//! Reference-output validation for the six Graphalytics algorithms. Outputs are
//! node-indexed (`out[node]`); reference files are `<vertex-id> <value>` per line,
//! so each check maps node → vertex via the [`Dataset`] before comparing. Modes
//! follow the spec: exact (BFS/CDLP), relabel-invariant (WCC), tolerance
//! (PageRank/LCC/SSSP).

use std::collections::HashMap;

use super::load::Dataset;

/// Parse a `<vertex-id> <value>` reference file into a vertex → raw-value map.
pub fn parse_reference(text: &str) -> HashMap<u32, String> {
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if let (Some(v), Some(val)) = (it.next(), it.next()) {
            if let Ok(vid) = v.parse::<u32>() {
                m.insert(vid, val.to_string());
            }
        }
    }
    m
}

/// Exact integer agreement (BFS depths, CDLP labels). Returns the first
/// disagreeing vertex, if any.
pub fn check_exact_i64(ds: &Dataset, out: &[i64], reference: &HashMap<u32, String>) -> Result<(), String> {
    for (node, &mine) in out.iter().enumerate() {
        let vertex = ds.vertex_of_node[node];
        let want = parse_ref::<i64>(reference, vertex)?;
        if mine != want {
            return Err(format!("vertex {vertex}: got {mine}, want {want}"));
        }
    }
    Ok(())
}

/// Tolerance float agreement (PageRank, LCC, SSSP); both-infinite (same sign)
/// passes, mixed finite/infinite fails.
pub fn check_epsilon(
    ds: &Dataset,
    out: &[f64],
    reference: &HashMap<u32, String>,
    eps: f64,
) -> Result<(), String> {
    for (node, &mine) in out.iter().enumerate() {
        let vertex = ds.vertex_of_node[node];
        let want = parse_ref::<f64>(reference, vertex)?;
        let ok = if mine.is_infinite() || want.is_infinite() {
            mine.is_infinite() && want.is_infinite() && mine.signum() == want.signum()
        } else {
            (mine - want).abs() <= eps
        };
        if !ok {
            return Err(format!("vertex {vertex}: got {mine}, want {want} (eps {eps})"));
        }
    }
    Ok(())
}

/// Relabel-invariant agreement (WCC): the partition `out` induces over vertices
/// must equal the reference's, regardless of the actual label values. Enforces a
/// consistent bijection between our labels and the reference's.
pub fn check_relabel(ds: &Dataset, out: &[u32], reference: &HashMap<u32, String>) -> Result<(), String> {
    let mut ours_to_ref: HashMap<u32, String> = HashMap::new();
    let mut ref_to_ours: HashMap<String, u32> = HashMap::new();
    for (node, &mine) in out.iter().enumerate() {
        let vertex = ds.vertex_of_node[node];
        let want = reference
            .get(&vertex)
            .ok_or_else(|| format!("vertex {vertex} missing from reference"))?
            .clone();
        if let Some(prev) = ours_to_ref.get(&mine) {
            if prev != &want {
                return Err(format!("vertex {vertex}: our label {mine} maps to both {prev} and {want}"));
            }
        } else {
            if let Some(&other) = ref_to_ours.get(&want) {
                if other != mine {
                    return Err(format!("vertex {vertex}: ref label {want} maps to both {other} and {mine}"));
                }
            }
            ours_to_ref.insert(mine, want.clone());
            ref_to_ours.insert(want, mine);
        }
    }
    Ok(())
}

/// Look up a vertex in the reference and parse its value, with descriptive errors.
fn parse_ref<T: std::str::FromStr>(reference: &HashMap<u32, String>, vertex: u32) -> Result<T, String> {
    let raw = reference.get(&vertex).ok_or_else(|| format!("vertex {vertex} missing from reference"))?;
    raw.parse::<T>().map_err(|_| format!("vertex {vertex}: unparseable reference value {raw:?}"))
}

#[cfg(test)]
mod tests {
    use super::super::load::load_str;
    use super::*;

    // Three vertices 1,2,3 -> dense nodes 0,1,2.
    fn ds() -> super::super::load::Dataset {
        load_str("1\n2\n3\n", "", "")
    }

    #[test]
    fn exact_passes_on_match_fails_on_diff() {
        let ds = ds();
        let r = parse_reference("1 0\n2 1\n3 2\n");
        assert!(check_exact_i64(&ds, &[0, 1, 2], &r).is_ok());
        assert!(check_exact_i64(&ds, &[0, 9, 2], &r).is_err());
    }

    #[test]
    fn epsilon_tolerates_small_drift_and_matches_infinity() {
        let ds = ds();
        let r = parse_reference("1 0.5\n2 0.25\n3 inf\n");
        assert!(check_epsilon(&ds, &[0.5 + 1e-9, 0.25, f64::INFINITY], &r, 1e-6).is_ok());
        // finite vs infinite is a mismatch.
        assert!(check_epsilon(&ds, &[0.5, 0.25, 1.0], &r, 1e-6).is_err());
        // out of tolerance.
        assert!(check_epsilon(&ds, &[0.6, 0.25, f64::INFINITY], &r, 1e-6).is_err());
    }

    #[test]
    fn relabel_accepts_renamed_partition_rejects_reshaped() {
        let ds = ds();
        // {1,2} together, {3} apart — labels 100/200 differ from ours but partition matches.
        let same = parse_reference("1 100\n2 100\n3 200\n");
        assert!(check_relabel(&ds, &[5, 5, 7], &same).is_ok());
        // Different partition: 2 and 3 grouped instead.
        let reshaped = parse_reference("1 100\n2 200\n3 200\n");
        assert!(check_relabel(&ds, &[5, 5, 7], &reshaped).is_err());
    }
}
