//! Reproducible sensitivity test for the synthetic routing benchmark.
//! This models network transfer only; it does not contact Storj or any nodes.
use darkrock_storage::routing::{Route, RouteStore};
use darkrock_storage::storage::{BLOCK_SIZE, DATA_SHARDS};

const NODES: [&str; 6] = ["us-east", "us-west", "eu-west", "eu-central", "ap-south", "ap-east"];
const EDGES: [(&str, &str); 14] = [
    ("replacement", "us-east"), ("replacement", "us-west"),
    ("replacement", "eu-west"), ("replacement", "eu-central"),
    ("replacement", "ap-south"), ("replacement", "ap-east"),
    ("replacement", "relay-us"), ("relay-us", "us-west"),
    ("replacement", "relay-eu"), ("relay-eu", "eu-west"),
    ("relay-eu", "eu-central"), ("replacement", "relay-ap"),
    ("relay-ap", "ap-south"), ("relay-ap", "ap-east"),
];
const TRIALS: usize = 1000;
const STRIPES: usize = 16;
const INBOUND_MIB_S: f64 = 25.0;
const SEED: u64 = 0x5eed_2026_0926;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 { lo + self.next() % (hi - lo + 1) }
}

#[derive(Clone, Copy)]
struct Link { a: &'static str, b: &'static str, latency_ms: f64, mib_s: f64 }
struct Topology { links: Vec<Link>, advertised: [f64; 6] }

fn topology(rng: &mut Rng) -> Topology {
    let links = EDGES.iter().map(|&(a, b)| Link {
        a, b,
        latency_ms: rng.range(10, 250) as f64,
        mib_s: rng.range(10, 100) as f64,
    }).collect();
    // Advertised rates deliberately differ from link rates, as in the original
    // scenario. This tests the proxy mismatch, not calibrated node telemetry.
    let advertised = std::array::from_fn(|_| rng.range(15, 110) as f64);
    Topology { links, advertised }
}

fn edge<'a>(topology: &'a Topology, a: &str, b: &str) -> &'a Link {
    topology.links.iter().find(|e| (e.a == a && e.b == b) || (e.b == a && e.a == b)).unwrap()
}

fn path(topology: &Topology, node: usize, spf: bool) -> Route {
    if !spf {
        let direct = edge(topology, "replacement", NODES[node]);
        return Route { nodes: vec!["replacement".into(), NODES[node].into()], total_cost: direct.latency_ms as u64 };
    }
    let mut graph = RouteStore::new("replacement", NODES[node]);
    for e in &topology.links { graph.add_link(e.a, e.b, e.latency_ms as u64).unwrap(); }
    graph.shortest_path().unwrap()
}

#[derive(Clone, Copy)]
enum Selection { Availability, Advertised, Spf }

fn network_ms(topology: &Topology, offline: &[usize], selection: Selection) -> (f64, usize) {
    let mut helpers: Vec<_> = (0..NODES.len()).filter(|i| !offline.contains(i))
        .map(|i| (i, path(topology, i, matches!(selection, Selection::Spf)))).collect();
    match selection {
        Selection::Availability => {} // Share index is the tie breaker.
        Selection::Advertised => helpers.sort_by(|a, b| topology.advertised[b.0].total_cmp(&topology.advertised[a.0])),
        Selection::Spf => helpers.sort_by_key(|(i, route)| (route.total_cost, *i)),
    }
    helpers.truncate(DATA_SHARDS);
    let hops = helpers.iter().map(|(_, route)| route.nodes.len() - 1).sum();
    let slowest = helpers.iter().map(|(i, route)| {
        let (latency, bottleneck) = route.nodes.windows(2).fold((0.0_f64, topology.advertised[*i]), |(lat, bw), pair| {
            let e = edge(topology, &pair[0], &pair[1]);
            (lat + e.latency_ms, bw.min(e.mib_s))
        });
        latency + BLOCK_SIZE as f64 / (1024.0 * 1024.0) / bottleneck * 1000.0
    }).fold(0.0_f64, f64::max);
    let inbound = DATA_SHARDS as f64 * BLOCK_SIZE as f64 / (1024.0 * 1024.0) / INBOUND_MIB_S * 1000.0;
    (STRIPES as f64 * slowest.max(inbound), hops)
}

#[derive(Default)]
struct Summary { spf_wins: usize, ties: usize, spf_losses: usize, ratios: Vec<f64>, hop_ratios: Vec<f64> }
impl Summary {
    fn add(&mut self, spf: (f64, usize), baseline: (f64, usize)) {
        let delta = spf.0 - baseline.0;
        if delta < -0.000001 { self.spf_wins += 1; }
        else if delta > 0.000001 { self.spf_losses += 1; }
        else { self.ties += 1; }
        self.ratios.push(spf.0 / baseline.0);
        self.hop_ratios.push(spf.1 as f64 / baseline.1 as f64);
    }
    fn print(&mut self, scenario: &str, baseline: &str) {
        self.ratios.sort_by(f64::total_cmp);
        self.hop_ratios.sort_by(f64::total_cmp);
        let n = self.ratios.len();
        println!("| {scenario} | {baseline} | {} | {} | {} | {:.3} | {:.3} | {:.3} |",
            self.spf_wins, self.ties, self.spf_losses,
            self.ratios[n / 2], self.ratios[(n * 95) / 100], self.hop_ratios[n / 2]);
    }
}

fn run() -> [[Summary; 2]; 2] {
    let mut rng = Rng(SEED);
    let mut summaries: [[Summary; 2]; 2] = std::array::from_fn(|_| std::array::from_fn(|_| Summary::default()));
    for _ in 0..TRIALS {
        let t = topology(&mut rng);
        let first = rng.range(0, 5) as usize;
        let mut second = rng.range(0, 4) as usize;
        if second >= first { second += 1; }
        for (scenario, offline) in [[first, first], [first, second]].into_iter().enumerate() {
            let lost: &[usize] = if scenario == 0 { &offline[..1] } else { &offline };
            let spf = network_ms(&t, lost, Selection::Spf);
            summaries[scenario][0].add(spf, network_ms(&t, lost, Selection::Availability));
            summaries[scenario][1].add(spf, network_ms(&t, lost, Selection::Advertised));
        }
    }
    summaries
}

fn main() {
    let mut summaries = run();
    println!("# Randomized routing sensitivity test\n");
    println!("{TRIALS} fixed-seed topologies; six share nodes, three relays, 4+2 shares, {STRIPES} sequential stripes. Direct and relay links each draw latency uniformly from 10–250 ms and bandwidth from 10–100 MiB/s. Advertised node rates draw independently from 15–110 MiB/s. Offline nodes are sampled uniformly. SPF minimizes summed link latency; transfer time uses summed latency and path bottleneck bandwidth.\n");
    println!("| Offline | SPF versus | Wins | Ties | Losses | Median time ratio | P95 time ratio | Median byte-hop ratio |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|");
    for (scenario, label) in ["one", "two"].iter().enumerate() {
        summaries[scenario][0].print(label, "availability/direct");
        summaries[scenario][1].print(label, "advertised/direct");
    }
    println!("\nRatios are SPF divided by baseline; below 1 is faster. The byte-hop ratio compares four helper paths and is a network-traffic proxy, not bytes downloaded for reconstruction. This is a synthetic model with independent flows, no shared-link congestion, no retries, no disk I/O, and no actual Storj telemetry or control over Storj repair. The model does not show whether Storj can expose this choice through its client API.");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeded_trials_are_reproducible_and_counted() {
        let a = run();
        let b = run();
        for scenario in 0..2 {
            for baseline in 0..2 {
                let x = &a[scenario][baseline];
                let y = &b[scenario][baseline];
                assert_eq!(x.spf_wins + x.ties + x.spf_losses, TRIALS);
                assert_eq!((x.spf_wins, x.ties, x.spf_losses), (y.spf_wins, y.ties, y.spf_losses));
                assert_eq!(x.ratios, y.ratios);
            }
        }
    }
}
