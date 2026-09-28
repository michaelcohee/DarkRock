use darkrock_storage::routing::{Route, RouteStore};
use darkrock_storage::storage::{BLOCK_SIZE, DATA_SHARDS, encode_stripe, reconstruct};
use std::time::Instant;

const MIB: f64 = 1024.0 * 1024.0;
const STRIPES: usize = 16;
const INBOUND_MIB_S: f64 = 25.0;

#[derive(Clone, Copy)]
struct ShareNode { name: &'static str, region: &'static str, advertised_mib_s: f64 }
#[derive(Clone, Copy)]
struct Link { a: &'static str, b: &'static str, cost: u64, latency_ms: f64, mib_s: f64 }

const NODES: [ShareNode; 6] = [
    ShareNode { name: "us-east", region: "US East", advertised_mib_s: 40.0 },
    ShareNode { name: "us-west", region: "US West", advertised_mib_s: 80.0 },
    ShareNode { name: "eu-west", region: "EU West", advertised_mib_s: 60.0 },
    ShareNode { name: "eu-central", region: "EU Central", advertised_mib_s: 50.0 },
    ShareNode { name: "ap-south", region: "AP South", advertised_mib_s: 90.0 },
    ShareNode { name: "ap-east", region: "AP East", advertised_mib_s: 100.0 },
];

// Link cost is an integer latency proxy for this controlled scenario. The
// advertised node rate and each actual path's bottleneck are separate values.
const LINKS: [Link; 14] = [
    Link { a: "replacement", b: "us-east", cost: 35, latency_ms: 35.0, mib_s: 40.0 },
    Link { a: "replacement", b: "us-west", cost: 105, latency_ms: 105.0, mib_s: 25.0 },
    Link { a: "replacement", b: "eu-west", cost: 145, latency_ms: 145.0, mib_s: 20.0 },
    Link { a: "replacement", b: "eu-central", cost: 120, latency_ms: 120.0, mib_s: 20.0 },
    Link { a: "replacement", b: "ap-south", cost: 225, latency_ms: 225.0, mib_s: 15.0 },
    Link { a: "replacement", b: "ap-east", cost: 185, latency_ms: 185.0, mib_s: 20.0 },
    Link { a: "replacement", b: "relay-us", cost: 10, latency_ms: 10.0, mib_s: 100.0 },
    Link { a: "relay-us", b: "us-west", cost: 25, latency_ms: 25.0, mib_s: 50.0 },
    Link { a: "replacement", b: "relay-eu", cost: 55, latency_ms: 55.0, mib_s: 70.0 },
    Link { a: "relay-eu", b: "eu-west", cost: 25, latency_ms: 25.0, mib_s: 30.0 },
    Link { a: "relay-eu", b: "eu-central", cost: 30, latency_ms: 30.0, mib_s: 30.0 },
    Link { a: "replacement", b: "relay-ap", cost: 130, latency_ms: 130.0, mib_s: 30.0 },
    Link { a: "relay-ap", b: "ap-south", cost: 55, latency_ms: 55.0, mib_s: 15.0 },
    Link { a: "relay-ap", b: "ap-east", cost: 40, latency_ms: 40.0, mib_s: 20.0 },
];

fn link(a: &str, b: &str) -> Link {
    *LINKS.iter().find(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a)).expect("known topology link")
}

fn shortest_path(to: &str) -> Route {
    let mut graph = RouteStore::new("replacement", to);
    for edge in LINKS { graph.add_link(edge.a, edge.b, edge.cost).unwrap(); }
    graph.shortest_path().unwrap()
}

fn direct_path(to: &str) -> Route {
    let edge = link("replacement", to);
    Route { nodes: vec!["replacement".into(), to.into()], total_cost: edge.cost }
}

fn transfer_ms(path: &Route, advertised_mib_s: f64) -> f64 {
    let mut latency = 0.0;
    let mut bottleneck = advertised_mib_s;
    for pair in path.nodes.windows(2) {
        let edge = link(&pair[0], &pair[1]);
        latency += edge.latency_ms;
        bottleneck = bottleneck.min(edge.mib_s);
    }
    latency + (BLOCK_SIZE as f64 / MIB) / bottleneck * 1000.0
}

#[derive(Clone)]
struct Helper { index: usize, path: Route }

#[derive(Clone, Copy)]
enum Selection { Availability, Bandwidth, Spf }
impl Selection {
    fn label(self) -> &'static str {
        match self { Self::Availability => "availability/hops", Self::Bandwidth => "bandwidth", Self::Spf => "SPF" }
    }
}

fn choose_helpers(offline: &[usize], selection: Selection) -> Option<Vec<Helper>> {
    let mut candidates: Vec<Helper> = (0..NODES.len()).filter(|i| !offline.contains(i)).map(|index| {
        let path = if matches!(selection, Selection::Spf) { shortest_path(NODES[index].name) } else { direct_path(NODES[index].name) };
        Helper { index, path }
    }).collect();
    if candidates.len() < DATA_SHARDS { return None; }
    match selection {
        Selection::Spf => candidates.sort_by_key(|h| (h.path.total_cost, h.index)),
        Selection::Bandwidth => candidates.sort_by(|a, b| NODES[b.index].advertised_mib_s.total_cmp(&NODES[a.index].advertised_mib_s)),
        Selection::Availability => {} // All direct paths have one hop; use share index to break ties.
    }
    candidates.truncate(DATA_SHARDS);
    Some(candidates)
}

fn cpu_repair_ms(offline: &[usize], stripes: &[darkrock_storage::storage::Stripe], original: &[u8]) -> f64 {
    let start = Instant::now();
    let mut output = Vec::with_capacity(original.len());
    for stripe in stripes {
        let mut shares: Vec<_> = stripe.shards.iter().cloned().map(Some).collect();
        for &index in offline { shares[index] = None; }
        output.extend(reconstruct(shares, &stripe.manifest).unwrap());
    }
    assert_eq!(output, original);
    start.elapsed().as_secs_f64() * 1000.0
}

fn report(scenario: &str, offline: &[usize], cpu_ms: Option<f64>, selection: Selection) {
    let Some(helpers) = choose_helpers(offline, selection) else {
        println!("| {} | {} | — | — | — | — | — | — | — | unrecoverable |", scenario, selection.label());
        return;
    };
    let helper_names = helpers.iter().map(|h| NODES[h.index].name).collect::<Vec<_>>().join(", ");
    let hops: usize = helpers.iter().map(|h| h.path.nodes.len() - 1).sum();
    let route_bytes: usize = helpers.iter().map(|h| 1 + h.path.nodes.len()).sum(); // one-byte count + one-byte node IDs
    let slowest_helper_ms = helpers.iter().map(|h| transfer_ms(&h.path, NODES[h.index].advertised_mib_s)).fold(0.0, f64::max);
    let inbound_ms = (DATA_SHARDS * BLOCK_SIZE) as f64 / MIB / INBOUND_MIB_S * 1000.0;
    let per_stripe_network_ms = slowest_helper_ms.max(inbound_ms);
    let modeled_ms = STRIPES as f64 * per_stripe_network_ms + cpu_ms.unwrap();
    let helper_bytes = STRIPES * DATA_SHARDS * BLOCK_SIZE;
    let written_bytes = STRIPES * offline.len() * BLOCK_SIZE;
    let byte_hops = STRIPES * BLOCK_SIZE * hops;
    println!("| {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.1} ms |", scenario,
        selection.label(), helper_names, hops, route_bytes, helper_bytes,
        written_bytes, byte_hops, format!("{per_stripe_network_ms:.1} ms"), modeled_ms);
}

fn main() {
    let mut x = 0x1234_5678_9abc_def0_u64;
    let original: Vec<u8> = (0..STRIPES * DATA_SHARDS * BLOCK_SIZE).map(|_| {
        x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8
    }).collect();
    let stripes: Vec<_> = original.chunks(DATA_SHARDS * BLOCK_SIZE).map(|c| encode_stripe(c).unwrap()).collect();
    println!("# Routing and repair-helper benchmark\n");
    println!("Six shares of each 4+2 stripe sit on six region-labelled nodes; relay edges have explicit integer costs, latency, and bandwidth. Availability/hops takes the first four live shares over one-hop direct links; bandwidth ranks advertised node upload rates over direct links; SPF ranks summed path cost and uses each shortest weighted path. Four helpers transfer a full 256 KiB share per stripe in parallel to a replacement capped at 25 MiB/s. Sixteen stripes represent 16 MiB logical input.\n");
    println!("| Loss scenario | Selection | Helpers | Hop sum | Route bytes | Helper bytes | Replacement bytes | Network byte-hops | Network/stripe | Modeled rebuild |");
    println!("|---|---|---|---:|---:|---:|---:|---:|---:|---:|");
    for (label, offline) in [("one", vec![0]), ("two", vec![0, 1]), ("three", vec![0, 1, 2])] {
        let cpu_ms = (offline.len() <= 2).then(|| cpu_repair_ms(&offline, &stripes, &original));
        for selection in [Selection::Availability, Selection::Bandwidth, Selection::Spf] {
            report(label, &offline, cpu_ms, selection);
        }
    }
    println!("\nHop sum counts the four selected helper paths; route bytes assume one-byte node IDs plus one-byte path length per helper, sent once. Network byte-hops count traffic over every traversed link. Helper bytes are fixed by the 4+2 codec and do not change with route choice. Modeled rebuild = 16 sequential stripe transfers plus measured local reconstruction wall time. Concurrent flows on the same relay link are treated independently; shared-link congestion, disk I/O, queueing, retries, and actual network traffic were not measured. This topology is a controlled example, not a Storj measurement.");
    println!("\n## Placement and paths\n");
    println!("| Share | Node | Region | Advertised rate |");
    println!("|---:|---|---|---:|");
    for (index, node) in NODES.iter().enumerate() {
        println!("| {} | {} | {} | {:.0} MiB/s |", index, node.name, node.region, node.advertised_mib_s);
    }
    for (label, offline) in [("one", vec![0]), ("two", vec![0, 1])] {
        for selection in [Selection::Availability, Selection::Bandwidth, Selection::Spf] {
            let helpers = choose_helpers(&offline, selection).unwrap();
            println!("- {} / {}: {}", label, selection.label(),
                helpers.iter().map(|h| h.path.nodes.join("→")).collect::<Vec<_>>().join("; "));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_changes_helper_choice_and_three_losses_fail() {
        let baseline = choose_helpers(&[0], Selection::Bandwidth).unwrap();
        let spf = choose_helpers(&[0], Selection::Spf).unwrap();
        assert_ne!(baseline.iter().map(|h| h.index).collect::<Vec<_>>(),
                   spf.iter().map(|h| h.index).collect::<Vec<_>>());
        assert!(choose_helpers(&[0, 1, 2], Selection::Spf).is_none());
    }
}
