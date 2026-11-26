# `tholos verify` — vérifier l'intégrité d'une archive

```text
tholos verify <ARCHIVE> [--quick [--sample N]] [--json]
```

## Vérification complète (défaut)

Décompresse chaque morceau et contrôle son empreinte BLAKE3, recalcule chaque racine de fichier et
la racine d'archive, puis établit le bilan par shards (parité, copies du trailer) et sa
réparabilité. C'est la seule vérification qui fait foi.

## Vérification rapide (`--quick`)

Pensée pour un contrôle fréquent (bibliothèque, tâche planifiée) d'archives volumineuses :

1. racines de fichier et racine d'archive recalculées depuis les tables, sans décompression ;
2. si l'archive porte une parité : CRC32C de chaque shard de données et de parité. Tout octet abîmé
   de la zone protégée (frames, index, tables, en-tête) est détecté sans décompression ;
3. `--sample N` morceaux (défaut 64) répartis uniformément sont décompressés et contrôlés par BLAKE3 ;
   l'échantillon est déterministe (mêmes morceaux à chaque exécution).

Si l'étape 2 trouve un shard abîmé, la vérification complète est exécutée automatiquement
(`dégât détecté par CRC : vérification complète effectuée`) : le verdict et la liste des morceaux
corrompus sont alors exactement ceux de `tholos verify`.

**Limite** : sans parité (`tholos pack --parity 0`), seul l'échantillon contrôle le contenu ; un
morceau abîmé hors échantillon n'est pas vu. La sortie le rappelle ; lancer `tholos verify` sans
`--quick` pour une vérification complète.

## Sorties

| Situation | Sortie | Code |
|---|---|---|
| Saine (rapide) | `OK (rapide) : 64 morceaux décompressés sur 9120, 312 fichiers, CRC de tous les shards conforme` | 0 |
| Saine (complète) | `OK : 9120 morceaux, 312 fichiers, parité présente` | 0 |
| Contenu intègre, parité ou trailer abîmés | `contenu intègre, redondance à restaurer (réparable)` | 0 |
| Contenu abîmé | `morceau 12 corrompu`, `fichier atteint : saves/slot1.dat` … `vérification échouée (réparable)` | 1 |

Un dégât réparable se corrige avec `tholos repair <ARCHIVE>`.

Chaque morceau corrompu est traduit en fichiers atteints (`fichier atteint : …`) : un morceau
dédupliqué atteint tous les fichiers qui le référencent.

## Sortie JSON (`--json`)

Schéma `tholos-verify/1`, destiné aux scripts, à la CI et aux écrans « Vérifier » et
« Bibliothèque » (statut saine / à réparer). Le JSON est écrit sur la sortie standard dans tous les
cas, y compris en échec ; le code de sortie est celui du tableau ci-dessus. Les clés ne sont
jamais renommées ; une évolution ajoute des clés ou change de schéma.

| Clé | Sens |
|---|---|
| `schema` | `"tholos-verify/1"` |
| `readable` | en-tête et tables lisibles ; `false` : seuls `error`, `ok`, `needs_repair`, `repairable` et `damage` sont présents |
| `mode` | `quick` ou `full` (`full` aussi quand la vérification rapide a escaladé) |
| `escalated` | la vérification rapide a vu un shard abîmé et a basculé en complète |
| `ok` | contenu intègre (morceaux, racines de fichier, racine d'archive) |
| `needs_repair` | un shard de données ou de parité, ou une copie du trailer, est abîmé |
| `repairable` | tout dégât constaté est reconstructible par `tholos repair` |
| `chunks_total`, `chunks_checked` | morceaux de l'archive, morceaux décompressés et contrôlés |
| `files` | nombre de fichiers |
| `bad_chunks` | indices des morceaux corrompus |
| `damaged_files` | chemins des fichiers référençant un morceau corrompu |
| `inconsistent_files` | chemins dont la racine ou la taille ne correspond pas aux tables |
| `archive_root_ok` | racine d'archive cohérente |
| `parity` | l'archive porte une parité |
| `damage` | `bad_data_shards[]`, `bad_parity_shards[]`, `bad_trailer_copies`, `parity_usable`, `overflow` (`{group, erased, parity}` ou `null`) |
| `archive_root` | racine d'archive (hex) |
