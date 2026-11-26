# crates/vault-api

API HTTP en lecture seule (Axum) sur un répertoire d'archives `.vault` ; binaire `tholos-api`.
Adaptateur mince : toute la logique de format est dans `vault-core`.

Documentation : [`docs/api/lecture.md`](../../docs/api/lecture.md). Choix de pile :
`docs/produit/stack.md` §5 (API Axum + front Next.js).

Tests : `cargo test -p vault-api` (routeur sans réseau, puis binaire réel sur un port éphémère).
