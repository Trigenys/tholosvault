# `tholos fetch` — récupérer une archive en vérifiant chaque morceau

`tholos fetch` récupère une archive `.vault` par plages alignées sur les morceaux, vérifie chaque
morceau par BLAKE3 à l'arrivée et reprend après une coupure sans relire un octet déjà vérifié
(ADR-0005). La source est distante (HTTP/HTTPS) ou **locale**.

## Sources

| Forme | Exemple | Transport |
|---|---|---|
| URL HTTP(S) | `https://exemple.org/jeux.vault` | `vault-fetch-http` (plages HTTP) |
| URL `file://` | `file:///mnt/nas/jeux%20retro.vault`, `file:///C:/Archives/jeux.vault` | fichier (`%XX` décodé) |
| Chemin | `/media/usb/jeux.vault`, `\\serveur\partage\jeux.vault`, `..\jeux.vault` | fichier |

Tout argument qui ne commence pas par `http://` ou `https://` est traité comme une source locale.

## Pourquoi passer par `fetch` plutôt que copier

Une copie brute (`cp`, Explorateur) depuis un disque externe ou un partage réseau ne vérifie rien et
repart de zéro après une coupure. Avec `tholos fetch` :

- chaque morceau est vérifié avant d'être conservé : un morceau abîmé sur la source est refusé, la copie
  n'est jamais déclarée complète ;
- une copie interrompue (partage déconnecté, clé USB retirée) reprend depuis l'état `<dest>.part` ;
- `--root <hex>` refuse la source avant toute copie de contenu si sa racine n'est pas celle attendue ;
- relancer sur une copie complète ne relit que le trailer (idempotent).

## Exemples

```sh
# Depuis un partage monté, vers le dossier courant (nom repris de la source)
tholos fetch /mnt/nas/jeux.vault

# Racine attendue, destination explicite, débit limité pour ménager le réseau
tholos fetch file:///mnt/nas/jeux.vault --out ~/archives/jeux.vault --root 4f2a… --limit 20M

# Plan sans rien écrire
tholos fetch /media/usb/jeux.vault --plan
```

## Garde-fous et erreurs

| Message | Cause | Action |
|---|---|---|
| `la destination … est le fichier source` | `--out` (ou le nom par défaut dans le dossier courant) désigne la source | choisir une autre destination avec `--out` |
| `source introuvable : …` | chemin erroné ou partage non monté | vérifier le chemin, monter le partage |
| `lecture refusée sur …` | droits du fichier ou du partage | corriger les droits ou l'utilisateur |
| `morceau N rejeté (BLAKE3)` puis échec | la source elle-même est abîmée | `tholos verify` puis `tholos repair` sur la source, ou une autre source |

Options communes (`--connections`, `--limit`, `--no-parity`, `--plan`, `--restart`, `--quiet`) : voir
`tholos fetch --help`.
