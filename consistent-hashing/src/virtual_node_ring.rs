use twox_hash::XxHash64;

/// Any change to this reshuffles every key in the ring
const HASH_SEED: u64 = 1;

/// Points per server. Imbalance shrinks like `1/sqrt(V)`, so 120 lands around
/// ±9%. libketama uses 160, Dynamo used ~100.
const VIRTUAL_NODES_PER_SERVER: usize = 120;

#[derive(Debug)]
struct VirtualNode {
    hash: u64,
    server_id: String,
}

/// Maps keys to servers so that adding or removing a server moves only the keys
/// that server owns
#[derive(Debug, Default)]
pub struct VirtualNodeRing {
    v_nodes: Vec<VirtualNode>,
}

impl VirtualNodeRing {
    pub fn new() -> VirtualNodeRing {
        VirtualNodeRing::default()
    }

    pub fn from_servers(server_ids: impl IntoIterator<Item = impl AsRef<str>>) -> VirtualNodeRing {
        let mut ring = VirtualNodeRing::new();
        for server_id in server_ids {
            ring.add(server_id.as_ref());
        }
        ring
    }

    /// Adding a server already in the ring replaces its points rather than
    /// stacking a second set on top, so callers never have to track membership
    /// themselves to avoid silently doubling a server's share.
    pub fn add(&mut self, server_id: &str) {
        self.add_weighted(server_id, 1);
    }

    /// `weight` multiplies this server's share of the ring, so a box with twice
    /// the capacity takes twice the keys.
    pub fn add_weighted(&mut self, server_id: &str, weight: usize) {
        self.remove(server_id);
        self.v_nodes
            .extend((0..VIRTUAL_NODES_PER_SERVER * weight).map(|i| VirtualNode {
                // Hash the salted label to scatter this server's points around
                // the ring, but store the bare id, since that is what callers
                // are asking for.
                hash: XxHash64::oneshot(HASH_SEED, format!("{server_id}#{i}").as_bytes()),
                server_id: server_id.to_owned(),
            }));
        self.v_nodes.sort_unstable_by_key(|vn| vn.hash);
    }

    /// `retain` compacts survivors leftward in order, so the sort survives and
    /// there is nothing to rebuild.
    pub fn remove(&mut self, server_id: &str) {
        self.v_nodes.retain(|vn| vn.server_id != server_id);
    }

    /// `None` only when the ring holds no servers at all.
    pub fn lookup(&self, key: &str) -> Option<&str> {
        if self.v_nodes.is_empty() {
            return None;
        }

        let h = XxHash64::oneshot(HASH_SEED, key.as_bytes());
        let idx = self.v_nodes.partition_point(|vn| vn.hash < h);
        // Walking clockwise past the largest point wraps back to the smallest,
        // which is the only thing making this a ring rather than a line.
        Some(&self.v_nodes[idx % self.v_nodes.len()].server_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const SERVERS: [&str; 3] = ["server-a", "server-b", "server-c"];

    fn tally(ring: &VirtualNodeRing, keys: usize) -> HashMap<&str, usize> {
        let mut counts = HashMap::new();
        for k in 0..keys {
            *counts
                .entry(ring.lookup(&format!("key-{k}")).unwrap())
                .or_default() += 1;
        }
        counts
    }

    #[test]
    fn membership_changes_leave_the_ring_consistent() {
        let mut ring = VirtualNodeRing::from_servers(SERVERS);

        ring.add("server-a");
        assert_eq!(ring.v_nodes.len(), SERVERS.len() * VIRTUAL_NODES_PER_SERVER);
        assert!(ring.v_nodes.is_sorted_by_key(|vn| vn.hash));

        ring.remove("server-b");
        assert!(ring.v_nodes.is_sorted_by_key(|vn| vn.hash));
        assert!((0..1000).all(|k| ring.lookup(&format!("key-{k}")) != Some("server-b")));
    }

    #[test]
    fn spreads_keys_evenly() {
        let ring = VirtualNodeRing::from_servers(SERVERS);
        let counts = tally(&ring, 100_000);
        let fair = 100_000 / SERVERS.len();
        for server in SERVERS {
            let got = counts[server];
            assert!(
                got.abs_diff(fair) < fair * 15 / 100,
                "{server} got {got}, fair share is {fair}"
            );
        }
    }

    #[test]
    fn weight_buys_proportional_share() {
        let mut ring = VirtualNodeRing::from_servers(SERVERS);
        ring.add_weighted("server-big", 2);
        let counts = tally(&ring, 100_000);
        assert!(counts["server-big"] > counts["server-a"] * 3 / 2);
    }

    #[test]
    fn adding_a_server_only_steals_its_own_keys() {
        let mut ring = VirtualNodeRing::from_servers(SERVERS);
        let before: Vec<(String, String)> = (0..10_000)
            .map(|k| format!("key-{k}"))
            .map(|key| {
                let owner = ring.lookup(&key).unwrap().to_owned();
                (key, owner)
            })
            .collect();

        ring.add("server-d");

        let mut moved = 0;
        for (key, old) in &before {
            let new = ring.lookup(key).unwrap();
            if new != old {
                assert_eq!(new, "server-d", "{key} moved between two existing servers");
                moved += 1;
            }
        }

        assert!(
            moved < before.len() * 30 / 100,
            "{moved} of {} keys moved",
            before.len()
        );
    }

    #[test]
    fn empty_ring_yields_nothing() {
        assert_eq!(VirtualNodeRing::new().lookup("key"), None);
    }
}
