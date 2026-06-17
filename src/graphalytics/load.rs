//! Graphalytics dataset loader: `<name>.v` / `<name>.e` / `<name>.properties`
//! into a [`GraphSnapshot`] plus the dense-node ↔ original-vertex id maps and the
//! per-algorithm run parameters (LDBC Graphalytics spec v1.0.x §2.3).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use rustychickpeas_core::{GraphBuilder, GraphSnapshot, PropertyValue};

use crate::harness::Result;

/// Per-dataset algorithm parameters parsed from `<name>.properties`. Sources are
/// original vertex ids (resolve to node ids via [`Dataset::node`]).
#[derive(Debug, Clone)]
pub struct Params {
    pub directed: bool,
    pub bfs_source: Option<u32>,
    pub sssp_source: Option<u32>,
    pub pr_damping: f64,
    pub pr_iterations: u32,
    pub cdlp_iterations: u32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            directed: true,
            bfs_source: None,
            sssp_source: None,
            pr_damping: 0.85,
            pr_iterations: 10,
            cdlp_iterations: 10,
        }
    }
}

/// A loaded dataset: the snapshot, its run parameters, and the maps between the
/// snapshot's dense node ids (`0..n`) and the dataset's original vertex ids.
pub struct Dataset {
    pub graph: GraphSnapshot,
    pub params: Params,
    /// node id -> original vertex id (algorithm output is node-indexed).
    pub vertex_of_node: Vec<u32>,
    /// original vertex id -> dense node id.
    pub node_of_vertex: HashMap<u32, u32>,
}

impl Dataset {
    /// Dense node id for an original vertex id.
    pub fn node(&self, vertex: u32) -> Option<u32> {
        self.node_of_vertex.get(&vertex).copied()
    }

    /// Number of vertices.
    pub fn len(&self) -> usize {
        self.vertex_of_node.len()
    }

    /// Whether the dataset has no vertices.
    pub fn is_empty(&self) -> bool {
        self.vertex_of_node.is_empty()
    }
}

/// Load `<dir>/<name>.{v,e,properties}`. A missing `.properties` falls back to
/// [`Params::default`].
pub fn load(dir: &Path, name: &str) -> Result<Dataset> {
    let v_text = fs::read_to_string(dir.join(format!("{name}.v")))?;
    let e_text = fs::read_to_string(dir.join(format!("{name}.e")))?;
    let props = fs::read_to_string(dir.join(format!("{name}.properties"))).unwrap_or_default();
    Ok(load_str(&v_text, &e_text, &props))
}

/// Build a [`Dataset`] from in-memory file contents (the unit-test seam for
/// [`load`]). Vertices are assigned dense node ids in `.v` file order; each `.e`
/// line `src dst [weight]` becomes an `e` relationship with a `weight` f64
/// property (default `1.0`). Undirected graphs store each edge once; algorithms
/// traverse `Direction::Both`. Edges referencing an unknown vertex are skipped.
pub fn load_str(v_text: &str, e_text: &str, props: &str) -> Dataset {
    let params = parse_params(props);

    let mut vertex_of_node: Vec<u32> = Vec::new();
    let mut node_of_vertex: HashMap<u32, u32> = HashMap::new();
    for line in v_text.lines() {
        let Some(tok) = line.split_whitespace().next() else {
            continue;
        };
        let Ok(vid) = tok.parse::<u32>() else {
            continue;
        };
        if !node_of_vertex.contains_key(&vid) {
            node_of_vertex.insert(vid, vertex_of_node.len() as u32);
            vertex_of_node.push(vid);
        }
    }

    let n = vertex_of_node.len();
    let mut b = GraphBuilder::new(Some(n), Some(e_text.lines().count()));
    for _ in 0..n {
        // add_node(None, ...) hands out dense ids 0..n in order, matching
        // vertex_of_node so node i corresponds to vertex_of_node[i].
        b.add_node(None, &["V"]).unwrap();
    }
    for line in e_text.lines() {
        let mut it = line.split_whitespace();
        let (Some(s), Some(d)) = (it.next(), it.next()) else {
            continue;
        };
        let (Ok(sv), Ok(dv)) = (s.parse::<u32>(), d.parse::<u32>()) else {
            continue;
        };
        let weight: f64 = it.next().and_then(|w| w.parse().ok()).unwrap_or(1.0);
        if let (Some(&su), Some(&du)) = (node_of_vertex.get(&sv), node_of_vertex.get(&dv)) {
            // Set the weight via the returned index: set_relationship_prop_f64 does
            // a linear find_rel_index scan, which is O(E^2) over a real edge list.
            let idx = b.add_relationship(su, du, "e").unwrap();
            b.set_relationship_props_by_index(idx, &[("weight", PropertyValue::Float(weight))]);
        }
    }

    let graph = b.finalize(None);
    Dataset { graph, params, vertex_of_node, node_of_vertex }
}

/// Parse the subset of LDBC `.properties` keys the six algorithms need, matching
/// on the key suffix so the dataset-name prefix (`graph.<name>.…`) is irrelevant.
fn parse_params(props: &str) -> Params {
    let mut p = Params::default();
    for line in props.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let (key, val) = (key.trim(), val.trim());
        if key.ends_with(".directed") {
            p.directed = val.eq_ignore_ascii_case("true");
        } else if key.ends_with(".bfs.source-vertex") {
            p.bfs_source = val.parse().ok();
        } else if key.ends_with(".sssp.source-vertex") {
            p.sssp_source = val.parse().ok();
        } else if key.ends_with(".pr.damping-factor") {
            if let Ok(d) = val.parse() {
                p.pr_damping = d;
            }
        } else if key.ends_with(".pr.num-iterations") {
            if let Ok(i) = val.parse() {
                p.pr_iterations = i;
            }
        } else if key.ends_with(".cdlp.max-iterations") {
            if let Ok(i) = val.parse() {
                p.cdlp_iterations = i;
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustychickpeas_core::Direction;

    const V: &str = "10\n20\n30\n";
    const E: &str = "10 20 2.5\n20 30 4.0\n10 30\n";
    const PROPS: &str = "\
graph.x.directed = false
graph.x.bfs.source-vertex = 20
graph.x.pr.damping-factor = 0.85
graph.x.pr.num-iterations = 7
graph.x.cdlp.max-iterations = 9
graph.x.sssp.source-vertex = 10
";

    #[test]
    fn maps_vertices_to_dense_nodes_in_order() {
        let ds = load_str(V, E, "");
        assert_eq!(ds.vertex_of_node, vec![10, 20, 30]);
        assert_eq!(ds.node(10), Some(0));
        assert_eq!(ds.node(30), Some(2));
        assert_eq!(ds.node(99), None);
        assert_eq!(ds.len(), 3);
    }

    #[test]
    fn edges_carry_weights_with_unit_default() {
        let ds = load_str(V, E, "");
        // node 0 (v10) -> node 1 (v20) and node 0 -> node 2 (v30, default weight).
        let outs: Vec<u32> = ds.graph.neighbors(0, Direction::Outgoing).collect();
        assert_eq!(outs.len(), 2);
        assert!(outs.contains(&1) && outs.contains(&2));
    }

    #[test]
    fn parses_algorithm_params() {
        let p = load_str(V, E, PROPS).params;
        assert!(!p.directed);
        assert_eq!(p.bfs_source, Some(20));
        assert_eq!(p.sssp_source, Some(10));
        assert_eq!(p.pr_damping, 0.85);
        assert_eq!(p.pr_iterations, 7);
        assert_eq!(p.cdlp_iterations, 9);
    }

    #[test]
    fn missing_properties_uses_defaults() {
        let p = load_str(V, E, "").params;
        assert!(p.directed);
        assert_eq!(p.pr_iterations, 10);
        assert_eq!(p.cdlp_iterations, 10);
        assert_eq!(p.bfs_source, None);
    }
}
