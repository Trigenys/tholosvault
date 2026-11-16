# Nom public — shortlist (décision D11)

Vérifications du 2026-11-16 : dépôts GitHub portant exactement le nom, organisation GitHub, crates.io, npm,
existence DNS des domaines (`NXDOMAIN` = probablement libre, à confirmer chez un registrar avant achat).
« VAULT » reste le nom de travail interne jusqu'à la décision.

| Nom | GitHub (dépôts homonymes) | Org GitHub | crates.io | npm | .com | .io | .dev | .app | .fr | Avis |
|---|---|---|---|---|---|---|---|---|---|---|
| **vaultfs** | asteris-llc/vaultfs ★131 (FUSE pour HashiCorp Vault, 2016, inactif), 2 autres mineurs | libre | libre | pris | pris | libre | libre | libre | libre | Le plus descriptif (« FS ») ; collision sémantique avec HashiCorp Vault à assumer |
| **chunkvault** | 3 dépôts ★0 | libre | libre | libre | pris | libre | libre | libre | libre | Dit « morceaux » ; aucune marque établie ; npm libre |
| **mountvault** | 1 dépôt ★1 | libre | libre | libre | pris | libre | libre | libre | libre | Dit « monter » ; neutre |
| **vaultarc** | 2 dépôts ★0–1 | libre | libre | libre | pris | libre | libre | libre | libre | Court, évoque « archive » ; moins explicite |
| **vaultmount** | 1 dépôt ★0 | libre | libre | libre | pris | libre | libre | libre | libre | Variante de mountvault, moins fluide |
| openvault | unkarelian/openvault ★45, WGBH openvault (archives TV) | **prise** | libre | pris | pris | pris | pris | pris | libre | Écarté : org GitHub et domaines pris |

Recommandation : **chunkvault** (aucune collision, npm + crates + org libres, `.io`/`.dev`/`.app` probablement libres)
ou **vaultfs** si l'on préfère la lisibilité immédiate malgré la proximité avec HashiCorp Vault.
Le binaire CLI peut rester `vault` dans tous les cas (crates.io : nom de crate ≠ nom de binaire), mais `vault` est déjà
le binaire HashiCorp : prévoir `vlt` ou le nom public comme commande.

Avant achat : vérifier la marque (INPI / EUIPO / USPTO) et le handle sur les réseaux utilisés.
