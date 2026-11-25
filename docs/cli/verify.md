# `tholos verify` — vérifier l'intégrité d'une archive

```text
tholos verify <ARCHIVE> [--quick [--sample N]]
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
| Contenu abîmé | `morceau 12 corrompu` … `vérification échouée (réparable)` | 1 |

Un dégât réparable se corrige avec `tholos repair <ARCHIVE>`.
