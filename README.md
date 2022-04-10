# Sorock

[Documentation](https://akiradeveloper.github.io/sorock/)

Sorock is an experimental "so rocking" scale-out distributed object storage.

The name comes from [Soroku Ebara](https://en.wikipedia.org/wiki/Ebara_Soroku), the founder of [Azabu high school](https://www.azabu-jh.ed.jp/) which is my alma mater.

![](https://upload.wikimedia.org/wikipedia/commons/c/c6/Soroku_Ebara.jpg)

## Features

- Containerized.
- gRPC API is defined for applications to access the storage.
- Erasure Coding is used for data resiliency.
- Cluster configuration is replicated by [Raft](https://github.akiradeveloper/lol) for faster propagation than Gossip.
- Automatic stabilization on cluster change.
- Automatic failure detection based on [On Scalable and Efficient Distributed Failure Detectors (2001)](https://dl.acm.org/doi/10.1145/383962.384010).
- Automatic data rebuild on node failure.

## Author

Akira Hayakawa (ruby.wktk@gmail.com)
