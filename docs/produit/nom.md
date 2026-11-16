# Nom public — tholosvault (décision D11, tranchée le 2026-11-16)

**Retenu : tholosvault**, signature « by Trigenys Group » (ADR-0002). *Tholos* (grec θόλος) : édifice circulaire à voûte, dont le
trésor de Delphes ; un tholos est littéralement une voûte. Binaire CLI : `tholos` (évite le conflit avec le `vault` de HashiCorp).
Vérifié le 2026-11-16 : aucun dépôt GitHub homonyme, organisation GitHub libre, crates.io et npm libres, `.com`, `.io`, `.dev`,
`.app` en NXDOMAIN. À faire avant la landing : acheter `tholosvault.com` (+ `.io`, `.dev`), réserver l'org GitHub, la crate et le
paquet npm, contrôler la marque (INPI / EUIPO).

## Deuxième tour : combinaisons gréco-latines (2026-11-16)

| Nom | Sens | Dépôts | Org | crates / npm | .com | .io/.dev/.app |
|---|---|---|---|---|---|---|
| **tholosvault** | tholos : voûte, trésor | aucun | libre | libres | libre | libres |
| arxvault | arx : citadelle | 1 ★0 | libre | libres | pris | libres |
| vaultarx | idem | aucun | libre | libres | pris | libres |
| custosvault | custos : gardien | aucun | libre | libres | pris | libres |
| aerariumvault | aerarium : trésor public | aucun | libre | libres | libre | libres |
| cellavault | cella : chambre du temple | aucun | prise | libres | libre | libres |
| janusvault, vestavault, vaultatlas | mythologie | 1–2 ★0 | libre | libres | pris | libres |
| arcavault, aegisvault, hermesvault | | collisions | prises | aegis pris | pris | pris |

## Premier tour : noms composés anglais

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
