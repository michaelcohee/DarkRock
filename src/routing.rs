//! OSPF-inspired weighted-path choice. It does not implement OSPF signaling.
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route { pub nodes: Vec<String>, pub total_cost: u64 }

#[derive(Clone, Debug)]
pub struct RouteStore {
    source: String,
    destination: String,
    links: HashMap<(String, String), u64>,
    candidates: Vec<Route>,
    best: Option<usize>,
    rejected: usize,
}

impl RouteStore {
    pub fn new(source: impl Into<String>, destination: impl Into<String>) -> Self {
        Self { source: source.into(), destination: destination.into(), links: HashMap::new(), candidates: Vec::new(), best: None, rejected: 0 }
    }
    pub fn add_link(&mut self, a: impl Into<String>, b: impl Into<String>, cost: u64) -> Result<(), String> {
        if cost == 0 { return Err("link cost must be positive".into()); }
        let (a, b) = (a.into(), b.into());
        self.links.insert((a.clone(), b.clone()), cost);
        self.links.insert((b, a), cost);
        Ok(())
    }
    pub fn ingest(&mut self, nodes: Vec<String>) -> Result<(), String> {
        let valid_ends = nodes.len() >= 2 && nodes.first() == Some(&self.source) && nodes.last() == Some(&self.destination);
        let unique = {
            let mut sorted = nodes.clone(); sorted.sort(); sorted.dedup(); sorted.len() == nodes.len()
        };
        if !valid_ends || !unique { self.rejected += 1; return Err("invalid route endpoints or repeated node".into()); }
        let mut cost = 0_u64;
        for pair in nodes.windows(2) {
            let link = self.links.get(&(pair[0].clone(), pair[1].clone())).ok_or_else(|| {
                self.rejected += 1; "route contains unknown link".to_string()
            })?;
            cost = cost.checked_add(*link).ok_or("route cost overflow")?;
        }
        let index = self.candidates.len();
        if self.best.map(|i| cost < self.candidates[i].total_cost).unwrap_or(true) { self.best = Some(index); }
        self.candidates.push(Route { nodes, total_cost: cost });
        Ok(())
    }
    pub fn best(&self) -> Option<&Route> { self.best.map(|i| &self.candidates[i]) }
    pub fn candidates(&self) -> &[Route] { &self.candidates }
    pub fn rejected(&self) -> usize { self.rejected }

    /// Dijkstra's shortest-path-first calculation over the known weighted links.
    pub fn shortest_path(&self) -> Option<Route> {
        let mut distances = HashMap::<String, u64>::new();
        let mut previous = HashMap::<String, String>::new();
        let mut heap = BinaryHeap::new();
        distances.insert(self.source.clone(), 0);
        heap.push(Reverse((0_u64, self.source.clone())));
        while let Some(Reverse((cost, node))) = heap.pop() {
            if cost != *distances.get(&node)? { continue; }
            if node == self.destination {
                let mut nodes = vec![node];
                while nodes.last()? != &self.source {
                    nodes.push(previous.get(nodes.last()?)?.clone());
                }
                nodes.reverse();
                return Some(Route { nodes, total_cost: cost });
            }
            for ((a, b), weight) in &self.links {
                if a != &node { continue; }
                let next = cost.checked_add(*weight)?;
                if next < *distances.get(b).unwrap_or(&u64::MAX) {
                    distances.insert(b.clone(), next);
                    previous.insert(b.clone(), node.clone());
                    heap.push(Reverse((next, b.clone())));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn longer_path_wins_on_lower_weight() {
        let mut store = RouteStore::new("A", "D");
        for (a, b, c) in [("A", "D", 12), ("A", "B", 2), ("B", "C", 2), ("C", "D", 2)] {
            store.add_link(a, b, c).unwrap();
        }
        store.ingest(vec!["A".into(), "D".into()]).unwrap();
        store.ingest(vec!["A".into(), "B".into(), "C".into(), "D".into()]).unwrap();
        assert_eq!(store.best().unwrap().total_cost, 6);
        assert_eq!(store.best().unwrap().nodes.len(), 4);
        assert_eq!(store.shortest_path(), store.best().cloned());
    }
}
