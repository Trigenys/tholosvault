# `tholos extract` — extraire tout ou partie d'une archive

```text
tholos extract <ARCHIVE> <RÉPERTOIRE> [CHEMIN…]
```

- Sans `CHEMIN` : toute l'archive est extraite dans `RÉPERTOIRE`.
- Avec un ou plusieurs `CHEMIN` : seuls ces fichiers sont extraits. Un chemin est soit le chemin
  exact d'un fichier, soit un répertoire (tous les fichiers sous `rép/`), tels que `tholos list` les
  affiche, sans barre initiale. `docs` sélectionne `docs/a.txt` mais pas `docs-hors.txt`.
- Un chemin sans correspondance ou qui sortirait de l'archive (`..`, `/…`) arrête la commande avant
  toute écriture.

## Garanties

- **Mémoire bornée** : chaque fichier est écrit morceau par morceau (`Archive::write_file_to` dans
  `vault-core`) ; un fichier de plusieurs gigaoctets n'est jamais chargé en entier.
- **Vérification** : chaque morceau est contrôlé par son empreinte BLAKE3 avant d'être écrit.
- **Pas de fichier tronqué** : l'écriture passe par `<cible>.tholos-part`, renommé en `<cible>`
  seulement une fois complet. Un morceau corrompu supprime le fichier partiel ; les autres fichiers
  sont extraits, et la commande se termine en erreur en listant les échecs.
- **Relançable** : après `tholos repair`, relancer la même commande réécrit les fichiers à
  l'identique.

## Exemples

```sh
tholos extract jeux.vault ./sortie                     # tout
tholos extract jeux.vault ./sortie saves/ config.ini   # un répertoire et un fichier
```

En cas d'échec (`échec : gros.bin : morceau 12 corrompu …`) : `tholos verify jeux.vault`, puis
`tholos repair jeux.vault` si la parité le permet, et relancer l'extraction.
