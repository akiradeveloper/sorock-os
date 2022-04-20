use crate::*;
use std::time::Duration;

pub struct Rebuild {
    pub cluster: ClusterMap,
    pub peer_out_cli: peer_out::ClientT,
    pub with_parity: bool,
    pub fallback_broadcast: bool,
}
impl Rebuild {
    pub async fn rebuild(self, key: String) -> anyhow::Result<Vec<Vec<u8>>> {
        let holders = self.cluster.compute_holders(key.clone(), N);
        let mut futs = vec![];
        for i in 0..N {
            let mut peer_out_cli = self.peer_out_cli.clone();
            let key = key.clone();
            let uri = holders[i as usize].clone();
            let fut = async move {
                match uri {
                    Some(uri) => peer_out_cli.request_any_pieces(uri, key).await,
                    None => {
                        anyhow::bail!("failed to compute the holder node key={}", &key);
                    }
                }
            };
            let fut = tokio::time::timeout(Duration::from_secs(5), fut);
            futs.push(fut);
        }

        let stream = futures::stream::iter(futs);
        let mut buffered = stream.buffer_unordered(N);
        let mut data = vec![None; N];
        let mut n_found = 0;
        while let Some(rep) = buffered.next().await {
            if rep.is_err() {
                continue;
            }
            let rep = rep.unwrap();
            if rep.is_err() {
                continue;
            }
            let pieces = rep.unwrap();
            for (i, piece_data) in pieces {
                if data[i as usize] == None {
                    data[i as usize] = Some(piece_data);
                    n_found += 1;
                }
            }
            if n_found >= K {
                use reed_solomon_erasure::galois_8::ReedSolomon;
                let r = ReedSolomon::new(K, N - K).unwrap();
                if self.with_parity {
                    r.reconstruct(&mut data).unwrap();
                } else {
                    r.reconstruct_data(&mut data).unwrap();
                    for _ in 0..(N - K) {
                        data.pop();
                    }
                }
                let mut out = vec![];
                for x in data {
                    out.push(x.unwrap());
                }
                return Ok(out);
            }
        }

        if !self.fallback_broadcast {
            anyhow::bail!("couldn't find enough piece without broadcasting.");
        }

        // broadcast (fallback)
        eprintln!("broadcast: {}", &key);
        let members = self.cluster.members();
        let mut futs = vec![];
        for uri in members {
            let mut peer_out_cli = self.peer_out_cli.clone();
            let key = key.clone();
            let fut = async move { peer_out_cli.request_any_pieces(uri, key).await };
            let fut = tokio::time::timeout(Duration::from_secs(5), fut);
            futs.push(fut);
        }
        let stream = futures::stream::iter(futs);
        let n_par = std::thread::available_parallelism().unwrap().get() * 2;
        let mut buffered = stream.buffer_unordered(n_par);
        let mut data = vec![None; N];
        let mut n_found = 0;
        while let Some(rep) = buffered.next().await {
            if rep.is_err() {
                continue;
            }
            let rep = rep.unwrap();
            if rep.is_err() {
                continue;
            }
            let pieces = rep.unwrap();
            for piece in pieces {
                let (index, piece_data) = piece;
                if data[index as usize] == None {
                    data[index as usize] = Some(piece_data);
                    n_found += 1;
                }
            }
            if n_found >= K {
                use reed_solomon_erasure::galois_8::ReedSolomon;
                let r = ReedSolomon::new(K, N - K).unwrap();
                if self.with_parity {
                    r.reconstruct(&mut data).unwrap();
                } else {
                    r.reconstruct_data(&mut data).unwrap();
                    for _ in 0..(N - K) {
                        data.pop();
                    }
                }
                let mut out = vec![];
                for x in data {
                    out.push(x.unwrap());
                }
                return Ok(out);
            }
        }

        anyhow::bail!("couldn't rebuild pieces (key={}). data is lost", key);
    }
}

#[test]
fn test_reed_solomon_huge_data() {
    use reed_solomon_erasure::galois_8::ReedSolomon;
    let r = ReedSolomon::new(4, 3).unwrap();
    let n = 28;
    let master = vec![1; 1 << (n + 2)]; // 1GB
    let data = vec![
        &master[0..1 << n],
        &master[1 << n..2 << n],
        &master[2 << n..3 << n],
        &master[3 << n..4 << n],
    ];
    let mut parity = vec![vec![0; 1 << n], vec![0; 1 << n], vec![0; 1 << n]];
    r.encode_sep(&data, &mut parity).unwrap();
    for i in 0..3 {
        assert_eq!(parity[i], vec![1; 1 << n]);
    }
}
