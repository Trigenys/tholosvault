# vault-mount

Montage **en lecture seule** d'une archive `.vault`. Le cœur est indépendant du système : il
expose l'archive comme un arbre d'inodes (répertoires implicites déduits des chemins, attributs,
parcours, lecture par plage avec cache de morceaux) et se teste sans pilote, sur toute cible. Les
adaptateurs de pilote sont de minces traducteurs d'appels et d'erreurs.

Toute logique de format reste dans `vault-core` : ce crate ne fait que servir ce que le lecteur lit.

## API (cœur, toujours disponible)

```rust
let fs = vault_mount::VaultFs::open("archive.vault")?;
let attr = fs.resolve("docs/guide.md")?;          // ou lookup(parent_ino, "guide.md")
let mut buf = vec![0u8; 128 * 1024];
let n = fs.read(attr.ino, 0, &mut buf)?;          // 0 au-delà de la fin
for e in fs.readdir(vault_mount::ROOT_INO, 0)? { /* ".", "..", puis entrées triées */ }
let st = fs.statfs();
```

- `VaultFs::open` / `from_archive` construisent l'arbre une fois ; `archive()` donne accès au lecteur.
- `getattr`, `lookup`, `resolve`, `readdir(ino, offset)` (reprise à un décalage), `read`, `statfs`.
- Erreurs `FsError` : `NotFound`, `NotDir`, `IsDir`, `Corrupt(_)` (un morceau dont le hachage ne
  correspond pas n'est **jamais** servi : erreur d'E/S côté noyau).
- Cache de morceaux décodés (`CACHE_CHUNKS` = 16) partagé par tous les lecteurs.

## Montage

```rust
let backend = vault_mount::probe()?;                // Err explicite sans adaptateur compilé
let handle = vault_mount::mount(fs, "/mnt/archive", &vault_mount::MountOptions::default())?;
handle.is_alive();
handle.unmount()?;
```

`MountOptions` : `threads` (défaut : parallélisme disponible), `allow_other`, `ttl` des attributs.

| Cible | Adaptateur | Feature Cargo | État |
|---|---|---|---|
| Linux | FUSE via `fuser` 0.18 (pur Rust, pas de libfuse) | `fuse` (par défaut) | disponible |
| Windows | WinFsp via `winfsp-rs` | `winfsp` | à venir (PR dédiée) |
| macOS | FSKit / FUSE-T | — | non planifié ici |

Sans adaptateur compilé pour la cible, `probe()` et `mount()` renvoient `MountError::NotCompiled`
avec la marche à suivre ; `tholos mount` affiche ce message et sort en erreur.

### Prérequis Linux

`fuser` sans la feature `libfuse` appelle `fusermount3` : installer `fuse3` (`apt install fuse3`).
Si le binaire n'est pas dans `PATH`/`/usr/sbin`, pointer `FUSERMOUNT_PATH` dessus. `/dev/fuse` doit
être accessible (c'est le cas sur les runners `ubuntu-latest` et dans WSL 2).

## Tests

- `cargo test -p vault-mount` : cœur sans pilote (arbre, attributs, lectures par morceaux, morceau
  corrompu refusé, sonde explicite). Tourne partout.
- `cargo test -p vault-mount --test fuse_mount -- --ignored` : montage réel sous Linux (`#[ignore]`
  hors CI ubuntu, qui l'exécute).
- Le banc (`bench/`, thèse H4) mesure le cœur partout et le montage réel là où l'hôte a un pilote.

## MSRV

`fuser` 0.18 impose Rust 1.85 ; ce crate, `vault-cli` et `bench` déclarent `rust-version = "1.85"`.
Le reste du workspace garde sa valeur.
