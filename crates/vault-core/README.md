# vault-core

Cœur du format tholosvault : découpage FastCDC, identifiants et arbres de Merkle BLAKE3, compression zstd par
morceau, index en clair lisible par mmap, trailer répliqué. Toute la logique de format vit ici ; les apps sont des
adaptateurs minces.

- Spécification : `docs/format/spec-v0.md`.
- API : `pack_dir` (écriture), `Archive::open` / `read_file` / `verify` (lecture).
- Tests : `cargo test -p vault-core` (unitaires + `tests/format_v0.rs`).
