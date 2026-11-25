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
- `xattr_names(ino)`, `xattr(ino, name)` : empreintes lues dans l'index (voir « Attributs étendus »).
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
| Windows | WinFsp via `winfsp-rs` 0.13 (`delayload`) | `winfsp` | disponible |
| Windows | WinFsp via `winfsp-rs` | `winfsp` | à venir (PR dédiée) |
| macOS | FSKit / FUSE-T | — | non planifié ici |

Sans adaptateur compilé pour la cible, `probe()` et `mount()` renvoient `MountError::NotCompiled`
avec la marche à suivre ; `tholos mount` affiche ce message et sort en erreur.

### Attributs étendus (Linux, FUSE)

Le montage expose en lecture les empreintes de l'index, sans décompression :

| Attribut | Nœud | Valeur |
|---|---|---|
| `user.tholos.root` | fichier | racine Merkle BLAKE3 du fichier (hex, 64 caractères) |
| `user.tholos.chunks` | fichier | nombre de morceaux référencés |
| `user.tholos.archive_root` | tout nœud | racine de l'archive montée |

```sh
getfattr -n user.tholos.root /mnt/jeux/data/niveau1.pak   # empreinte sans lire le fichier
getfattr -d -m '^user\.tholos\.' /mnt/jeux                 # racine de l'archive
```

Deux fichiers de même `user.tholos.root` ont le même contenu, y compris d'une archive à l'autre :
un script compare deux versions montées sans lire un octet de contenu. Les valeurs sont celles que
l'index déclare ; `tholos verify` les contrôle contre les données. Attribut absent : `ENODATA`.
Sous Windows (WinFsp), les attributs étendus ne sont pas encore exposés.

### Prérequis Linux

`fuser` sans la feature `libfuse` appelle `fusermount3` : installer `fuse3` (`apt install fuse3`).
Si le binaire n'est pas dans `PATH`/`/usr/sbin`, pointer `FUSERMOUNT_PATH` dessus. `/dev/fuse` doit
être accessible (c'est le cas sur les runners `ubuntu-latest` et dans WSL 2).

### Prérequis Windows

- **À l'exécution** : le pilote [WinFsp](https://winfsp.dev) (`winget install WinFsp.WinFsp`, ou le MSI
  du site). La DLL est liée en différé (`/DELAYLOAD`) : sans pilote, le binaire démarre et `probe()`
  renvoie `DriverUnavailable` avec le conseil d'installation (la sonde lit `InstallDir` dans
  `HKLM\SOFTWARE\WinFsp`, charge la DLL par son chemin complet, puis initialise WinFsp).
  **Tout binaire qui embarque `vault-mount` avec `winfsp`** (`tholos`, `tholos-bench`, une app)
  doit appeler `winfsp::build::winfsp_link_delayload()` dans son propre `build.rs` : l'argument de
  liaison ne se propage pas depuis la bibliothèque, et sans lui le binaire refuse de démarrer
  (`STATUS_DLL_NOT_FOUND`) sur un poste sans pilote. La CI le vérifie (`tholos --version` avant
  l'installation de WinFsp).
- **À la compilation** : `winfsp-sys` génère ses liaisons avec `bindgen`, qui exige **libclang**
  (LLVM : `winget install LLVM.LLVM`, puis `LIBCLANG_PATH` vers son dossier `bin` si `clang.dll` n'est
  pas trouvée ; sans installation, la roue PyPI `libclang` décompressée fournit une `libclang.dll`
  portable à pointer par `LIBCLANG_PATH`). Aucun SDK WinFsp n'est requis : la bibliothèque d'import
  est embarquée dans le crate.
  Les runners `windows-latest` ont déjà LLVM.
- **Point de montage** : une lettre de lecteur libre (`X:`) ou un dossier vide ou inexistant. WinFsp
  prend le dossier en charge et le **supprime au démontage**.
- **Sémantique** : volume en lecture seule, insensible à la casse avec casse d'origine préservée
  (comportement attendu par l'Explorateur) ; la correspondance insensible est faite dans l'adaptateur,
  le cœur reste sensible à la casse. La racine n'expose pas `.` ni `..` (convention WinFsp). Toute
  ouverture en écriture, création ou écriture reçoit `STATUS_MEDIA_WRITE_PROTECTED`.
- **Licence** : `winfsp-rs` est publié sous GPL-3.0 (WinFsp lui-même sous GPLv3 avec exception FLOSS) ;
  l'adaptateur n'est compilé que sous Windows avec la feature `winfsp`.

## Tests

- `cargo test -p vault-mount` : cœur sans pilote (arbre, attributs, lectures par morceaux, morceau
  corrompu refusé, sonde explicite). Tourne partout.
- `cargo test -p vault-mount --test fuse_mount -- --ignored` : montage réel sous Linux, lecture et
  attributs étendus via `getxattr(2)`/`listxattr(2)` (`#[ignore]` hors CI ubuntu, qui l'exécute).
- `cargo test -p vault-mount --features winfsp --test winfsp_mount -- --ignored` : montage réel sous
  Windows (`#[ignore]` ; la CI windows installe WinFsp 2.2.26323 et l'exécute).
- Le banc (`bench/`, thèse H4) mesure le cœur partout et le montage réel là où l'hôte a un pilote.

## MSRV

`fuser` 0.18 et `winfsp` 0.13 (edition 2024) imposent Rust 1.85 ; ce crate, `vault-cli` et `bench`
déclarent `rust-version = "1.85"`.
Le reste du workspace garde sa valeur.
