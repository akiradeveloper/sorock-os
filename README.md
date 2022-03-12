# Sorock

[Documentation](https://akiradeveloper.github.io/sorock/)

Sorock is an experimental "so rocking" scale-out distributed object storage.

The name comes from [Soroku Ebara](https://en.wikipedia.org/wiki/Ebara_Soroku), the founder of [Azabu high school](https://www.azabu-jh.ed.jp/) which is my alma mater.

![](https://upload.wikimedia.org/wikipedia/commons/c/c6/Soroku_Ebara.jpg)

## Features

- Containerized.
- Erasure Coding for data resiliency.
- [ASURA](https://github.com/akiradeveloper/ASURA) instead of consistent-hashing for calculating holder nodes.
- Cluster configuration is replicated by [Raft](https://github.akiradeveloper/lol) for faster propagation than Gossip.
- Automatic stabilization on cluster change.
- Automatic failure detection by SWIM.
- Automatic data rebuild on node failure.

## Author

Akira Hayakawa (ruby.wktk@gmail.com)
