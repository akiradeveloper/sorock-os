use crate::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

struct Inner {
    version: u64,
    cluster: asura::Cluster,
    idmap: HashMap<u64, URI>,
}

#[derive(Clone)]
pub struct ClusterMap {
    inner: Arc<Inner>,
}
impl ClusterMap {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                version: 0,
                cluster: asura::Cluster::new(),
                idmap: HashMap::new(),
            }),
        }
    }
    pub(crate) fn build(version: u64, cluster: asura::Cluster, idmap: HashMap<u64, URI>) -> Self {
        let inner = Inner {
            version,
            cluster,
            idmap,
        };
        Self {
            inner: Arc::new(inner),
        }
    }
    pub fn version(&self) -> u64 {
        self.inner.version
    }
    pub fn members(&self) -> HashSet<Uri> {
        let mut out = HashSet::new();
        for (_, uri) in &self.inner.idmap {
            out.insert(uri.0.clone());
        }
        out
    }
    pub fn compute_holders(&self, key: String, n: usize) -> Vec<Option<Uri>> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut s = DefaultHasher::new();
        key.hash(&mut s);
        let data_key = s.finish();

        match self.inner.cluster.calc_candidates(data_key, n) {
            Some(ids) => {
                let m = ids.len();
                let mut out = vec![];
                for i in 0..n {
                    let id = ids[i % m];
                    let uri = self.inner.idmap.get(&id).unwrap().clone().0;
                    out.push(Some(uri));
                }
                out
            }
            None => {
                let mut out = vec![];
                for _ in 0..n {
                    out.push(None)
                }
                out
            }
        }
    }
}
