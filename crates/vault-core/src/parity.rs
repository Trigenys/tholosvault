//! Parité Reed-Solomon du format v1 (L-004).
//!
//! La zone protégée couvre tout le début de l'archive — en-tête, frames zstd
//! adressées, index des morceaux, table des fichiers, table de hachage — jusqu'au
//! début de la section de parité. Elle est découpée en *shards* de taille fixe
//! (4 KiB par défaut), répartis en groupes **entrelacés** : le shard `i`
//! appartient au groupe `i mod G`, en position `i / G`. Une perte contiguë de
//! `R` shards ne touche donc chaque groupe qu'au plus `⌈R / G⌉` fois.
//!
//! Chaque groupe reçoit `m` shards de parité (GF(2^16), crate `reed-solomon-simd`),
//! `m = ⌈k_max × budget / 100⌉ + 1` : le shard supplémentaire absorbe le
//! désalignement d'une perte contiguë de `budget %` de la zone protégée.
//!
//! La section est en clair et auto-vérifiée :
//!
//! ```text
//! ┌──────────────┬──────────────────┬────────────────────┬──────────────────────┐
//! │ en-tête (48) │ CRC32C × N shards│ CRC32C × P parités │ P shards de parité   │
//! └──────────────┴──────────────────┴────────────────────┴──────────────────────┘
//! ```
//!
//! Les CRC par shard localisent les effacements ; les hachages BLAKE3 des morceaux
//! servent de repli (un morceau au hachage faux dont aucun shard n'est signalé
//! voit tous ses shards marqués effacés). La parité `j` du groupe `g` est stockée
//! au slot `j × G + g`, elle aussi entrelacée.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ops::Range;

use reed_solomon_simd::ReedSolomonEncoder;

use crate::error::{Error, Result};
use crate::format::{u32_at, u64_at};

pub const PARITY_MAGIC: [u8; 8] = *b"THOLOSPR";
pub const PARITY_HEADER_LEN: usize = 48;
pub const DEFAULT_PERCENT: u8 = 5;
pub const DEFAULT_SHARD_SIZE: u32 = 4096;
pub const DEFAULT_GROUP_MAX: u32 = 1024;
pub const MIN_SHARD_SIZE: u32 = 64;
pub const MAX_SHARD_SIZE: u32 = 1 << 20;
pub const MAX_GROUP: u32 = 32_768;

/// Paramètres de parité choisis à l'écriture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParityOptions {
    /// Budget en pourcentage de la zone protégée ; 0 désactive la parité.
    pub percent: u8,
    /// Taille d'un shard en octets (paire, 64 o à 1 MiB).
    pub shard_size: u32,
    /// Nombre maximal de shards de données par groupe.
    pub group_max: u32,
}

impl Default for ParityOptions {
    fn default() -> Self {
        Self {
            percent: DEFAULT_PERCENT,
            shard_size: DEFAULT_SHARD_SIZE,
            group_max: DEFAULT_GROUP_MAX,
        }
    }
}

impl ParityOptions {
    pub fn disabled() -> Self {
        Self {
            percent: 0,
            ..Self::default()
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.percent > 0
    }

    pub fn validate(&self) -> Result<()> {
        if self.percent > 100 {
            return Err(Error::BadParityParams("budget supérieur à 100 %"));
        }
        if self.shard_size % 2 != 0 || !(MIN_SHARD_SIZE..=MAX_SHARD_SIZE).contains(&self.shard_size)
        {
            return Err(Error::BadParityParams(
                "taille de shard hors plage (paire, 64 o à 1 MiB)",
            ));
        }
        if !(1..=MAX_GROUP).contains(&self.group_max) {
            return Err(Error::BadParityParams(
                "taille de groupe hors plage (1 à 32768)",
            ));
        }
        Ok(())
    }
}

/// Géométrie d'une section, dérivée de la longueur protégée et des options ;
/// recalculée identiquement à l'écriture et à la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub percent: u8,
    pub shard_size: u32,
    pub group_max: u32,
    pub protected_len: u64,
    /// N : shards de données (le dernier est complété de zéros pour le calcul).
    pub data_shards: u64,
    /// G : groupes entrelacés.
    pub groups: u64,
    /// m : shards de parité par groupe.
    pub recovery_per_group: u32,
}

impl Geometry {
    pub fn new(protected_len: u64, opts: &ParityOptions) -> Self {
        let s = opts.shard_size as u64;
        let data_shards = protected_len.div_ceil(s).max(1);
        let groups = data_shards.div_ceil(opts.group_max as u64);
        let k_max = data_shards.div_ceil(groups);
        let recovery = (k_max * opts.percent as u64).div_ceil(100) + 1;
        Self {
            percent: opts.percent,
            shard_size: opts.shard_size,
            group_max: opts.group_max,
            protected_len,
            data_shards,
            groups,
            recovery_per_group: recovery as u32,
        }
    }

    /// (groupe, position dans le groupe) du shard de données `i`.
    pub fn locate(&self, shard: u64) -> (u64, u64) {
        (shard % self.groups, shard / self.groups)
    }

    pub fn shard_of_group(&self, group: u64, pos: u64) -> u64 {
        pos * self.groups + group
    }

    /// k_g : shards de données du groupe `g`.
    pub fn data_shards_in_group(&self, group: u64) -> u64 {
        let n = self.data_shards;
        n / self.groups + u64::from(group < n % self.groups)
    }

    /// P : shards de parité au total.
    pub fn parity_shards(&self) -> u64 {
        self.groups * self.recovery_per_group as u64
    }

    /// Slot de stockage de la parité `j` du groupe `g` (entrelacé).
    pub fn parity_slot(&self, group: u64, j: u64) -> u64 {
        j * self.groups + group
    }

    pub fn parity_slot_group(&self, slot: u64) -> u64 {
        slot % self.groups
    }

    pub fn section_len(&self) -> u64 {
        PARITY_HEADER_LEN as u64
            + 4 * self.data_shards
            + 4 * self.parity_shards()
            + self.parity_shards() * self.shard_size as u64
    }

    /// Octets réels (non complétés) du shard de données `i` dans la zone protégée.
    pub fn data_range(&self, shard: u64) -> Range<usize> {
        let s = self.shard_size as u64;
        let start = shard * s;
        let end = (start + s).min(self.protected_len);
        start as usize..end as usize
    }

    /// Shards de données recouvrant un intervalle d'octets de la zone protégée.
    pub fn shards_covering(&self, range: Range<u64>) -> Range<u64> {
        if range.is_empty() {
            return 0..0;
        }
        let s = self.shard_size as u64;
        range.start / s..(range.end - 1) / s + 1
    }

    fn encode_header(&self, crc_tables: u32) -> [u8; PARITY_HEADER_LEN] {
        let mut b = [0u8; PARITY_HEADER_LEN];
        b[0..8].copy_from_slice(&PARITY_MAGIC);
        b[8..12].copy_from_slice(&self.shard_size.to_le_bytes());
        b[12..16].copy_from_slice(&self.group_max.to_le_bytes());
        b[16..20].copy_from_slice(&self.recovery_per_group.to_le_bytes());
        b[20] = self.percent;
        b[24..32].copy_from_slice(&self.data_shards.to_le_bytes());
        b[32..40].copy_from_slice(&self.protected_len.to_le_bytes());
        b[40..44].copy_from_slice(&crc_tables.to_le_bytes());
        let crc = crc32c::crc32c(&b[..PARITY_HEADER_LEN - 4]);
        b[PARITY_HEADER_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
        b
    }
}

fn malformed(detail: impl Into<String>) -> Error {
    Error::MalformedSection {
        section: "parité",
        detail: detail.into(),
    }
}

fn rs_err(e: reed_solomon_simd::Error) -> Error {
    Error::ReedSolomon(e.to_string())
}

/// Vue zéro-copie d'une section de parité validée (en-tête et tables intactes).
pub struct ParitySection<'a> {
    pub geometry: Geometry,
    data_crc: &'a [u8],
    parity_crc: &'a [u8],
    shards: &'a [u8],
}

impl<'a> ParitySection<'a> {
    /// Valide la section contre la longueur protégée annoncée par le trailer.
    pub fn parse(bytes: &'a [u8], protected_len: u64) -> Result<Self> {
        if bytes.len() < PARITY_HEADER_LEN || bytes[0..8] != PARITY_MAGIC {
            return Err(malformed("en-tête absent"));
        }
        let h = &bytes[..PARITY_HEADER_LEN];
        if crc32c::crc32c(&h[..PARITY_HEADER_LEN - 4]) != u32_at(h, PARITY_HEADER_LEN - 4) {
            return Err(malformed("CRC de l'en-tête invalide"));
        }
        let opts = ParityOptions {
            percent: h[20],
            shard_size: u32_at(h, 8),
            group_max: u32_at(h, 12),
        };
        opts.validate()?;
        if !opts.is_enabled() {
            return Err(malformed("budget nul"));
        }
        let geometry = Geometry::new(protected_len, &opts);
        if u64_at(h, 32) != protected_len
            || u64_at(h, 24) != geometry.data_shards
            || u32_at(h, 16) != geometry.recovery_per_group
        {
            return Err(malformed("géométrie incohérente avec le trailer"));
        }
        if geometry.section_len() != bytes.len() as u64 {
            return Err(malformed(format!(
                "{} octets pour une géométrie de {}",
                bytes.len(),
                geometry.section_len()
            )));
        }
        let n = geometry.data_shards as usize;
        let p = geometry.parity_shards() as usize;
        let tables_end = PARITY_HEADER_LEN + 4 * n + 4 * p;
        if crc32c::crc32c(&bytes[PARITY_HEADER_LEN..tables_end]) != u32_at(h, 40) {
            return Err(malformed("CRC des tables invalide"));
        }
        Ok(Self {
            geometry,
            data_crc: &bytes[PARITY_HEADER_LEN..PARITY_HEADER_LEN + 4 * n],
            parity_crc: &bytes[PARITY_HEADER_LEN + 4 * n..tables_end],
            shards: &bytes[tables_end..],
        })
    }

    pub fn data_crc(&self, shard: u64) -> u32 {
        u32_at(self.data_crc, 4 * shard as usize)
    }

    pub fn parity_crc(&self, slot: u64) -> u32 {
        u32_at(self.parity_crc, 4 * slot as usize)
    }

    pub fn parity_shard(&self, slot: u64) -> &'a [u8] {
        let s = self.geometry.shard_size as usize;
        &self.shards[slot as usize * s..(slot as usize + 1) * s]
    }

    /// Offset, relatif au début de la section, du shard de parité `slot`.
    pub fn parity_shard_offset(&self, slot: u64) -> u64 {
        PARITY_HEADER_LEN as u64
            + 4 * self.geometry.data_shards
            + 4 * self.geometry.parity_shards()
            + slot * self.geometry.shard_size as u64
    }
}

fn padded_shard<'a>(protected: &'a [u8], geom: &Geometry, shard: u64) -> Cow<'a, [u8]> {
    let r = geom.data_range(shard);
    let s = geom.shard_size as usize;
    if r.len() == s {
        Cow::Borrowed(&protected[r])
    } else {
        let mut v = vec![0u8; s];
        v[..r.len()].copy_from_slice(&protected[r]);
        Cow::Owned(v)
    }
}

/// Applique `f` à `0..n` et renvoie les résultats dans l'ordre. Avec la
/// fonctionnalité `parallel`, le travail est réparti sur le pool rayon ; le
/// résultat est identique dans les deux cas.
fn map_ordered<T, F>(n: u64, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(u64) -> T + Sync + Send,
{
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        (0..n).into_par_iter().map(f).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        (0..n).map(f).collect()
    }
}

/// Comme [`map_ordered`], avec un état par travailleur créé par `init` et
/// prêté à chaque appel (ici : un encodeur dont l'espace de travail est réutilisé).
fn map_ordered_init<S, T, I, F>(n: u64, init: I, f: F) -> Vec<T>
where
    T: Send,
    I: Fn() -> S + Sync + Send,
    F: Fn(&mut S, u64) -> T + Sync + Send,
{
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        (0..n).into_par_iter().map_init(init, f).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut state = init();
        (0..n).map(|i| f(&mut state, i)).collect()
    }
}

/// Ensemble de travail visé par un appel d'encodage (données et secours d'une
/// tranche de colonnes) : tient dans le cache L2, ce qui double le débit mono-cœur.
const ENCODE_WORKING_SET: usize = 1 << 20;

/// Largeur d'une tranche de colonnes : un diviseur de `s`, multiple de 64 octets,
/// tel que `(k + m) × largeur` tienne dans [`ENCODE_WORKING_SET`] ; `s` sinon.
///
/// Le code de Reed-Solomon est appliqué indépendamment à chaque bloc de 64 octets
/// d'un shard : encoder une tranche de colonnes donne exactement la tranche
/// correspondante des shards de secours (vérifié par `slicing_is_exact`).
fn slice_width(s: usize, k: usize, m: usize) -> usize {
    let mut w = s;
    while w % 128 == 0 && (k + m) * w > ENCODE_WORKING_SET {
        w /= 2;
    }
    w
}

/// Encode les `m` shards de secours du groupe `g`, restreints aux colonnes
/// `[part × w, (part + 1) × w)`, dans `enc` (réinitialisé, espace réutilisé).
/// Renvoie les `m` tranches concaténées (`m × w` octets). Les tâches sont
/// indépendantes entre elles.
fn encode_slice(
    enc: &mut ReedSolomonEncoder,
    protected: &[u8],
    geom: &Geometry,
    g: u64,
    part: usize,
    w: usize,
    m: usize,
) -> Result<Vec<u8>> {
    let k = geom.data_shards_in_group(g);
    let s = geom.shard_size as usize;
    enc.reset(k as usize, m, w).map_err(rs_err)?;
    let col = |r: Range<usize>| r.start + part * w..r.start + (part + 1) * w;
    // Seul le dernier shard de la zone (dernière position de son groupe) peut
    // être incomplet ; ailleurs on lit la zone protégée sans copie.
    let last = geom.data_range(geom.shard_of_group(g, k - 1));
    if last.len() == s {
        for pos in 0..k {
            enc.add_original_shard(&protected[col(geom.data_range(geom.shard_of_group(g, pos)))])
                .map_err(rs_err)?;
        }
    } else {
        for pos in 0..k {
            let sh = padded_shard(protected, geom, geom.shard_of_group(g, pos));
            enc.add_original_shard(&sh[part * w..(part + 1) * w])
                .map_err(rs_err)?;
        }
    }
    let res = enc.encode().map_err(rs_err)?;
    let mut out = vec![0u8; m * w];
    for (j, r) in res.recovery_iter().enumerate() {
        out[j * w..(j + 1) * w].copy_from_slice(r);
    }
    Ok(out)
}

/// Construit la section de parité d'une zone protégée. Déterministe.
pub fn build_section(protected: &[u8], opts: &ParityOptions) -> Result<Vec<u8>> {
    opts.validate()?;
    let geom = Geometry::new(protected.len() as u64, opts);
    let n = geom.data_shards as usize;
    let p = geom.parity_shards() as usize;
    let s = geom.shard_size as usize;
    let m = geom.recovery_per_group as usize;

    // Disposition : en-tête, CRC des données, CRC de la parité, shards de parité.
    let tables_off = PARITY_HEADER_LEN;
    let parity_crc_off = tables_off + 4 * n;
    let shards_off = parity_crc_off + 4 * p;
    let mut out = vec![0u8; geom.section_len() as usize];

    let data_crc = map_ordered(n as u64, |i| crc32c::crc32c(&protected[geom.data_range(i)]));
    for (i, c) in data_crc.iter().enumerate() {
        out[tables_off + 4 * i..tables_off + 4 * i + 4].copy_from_slice(&c.to_le_bytes());
    }

    let w = slice_width(s, geom.group_max as usize, m);
    let split = s / w;
    let k0 = geom.data_shards_in_group(0) as usize;
    let tasks = map_ordered_init(
        geom.groups * split as u64,
        || ReedSolomonEncoder::new(k0, m, w),
        |enc, t| {
            let (g, part) = (t / split as u64, (t % split as u64) as usize);
            let enc = enc.as_mut().map_err(|e| rs_err(*e))?;
            encode_slice(enc, protected, &geom, g, part, w, m)
        },
    );
    for (t, recovery) in tasks.into_iter().enumerate() {
        let (g, part) = (t / split, t % split);
        let recovery = recovery?;
        for j in 0..m {
            let slot = geom.parity_slot(g as u64, j as u64) as usize;
            let at = shards_off + slot * s + part * w;
            out[at..at + w].copy_from_slice(&recovery[j * w..(j + 1) * w]);
        }
    }

    let shards = &out[shards_off..];
    let parity_crc = map_ordered(p as u64, |slot| {
        let slot = slot as usize;
        crc32c::crc32c(&shards[slot * s..(slot + 1) * s])
    });
    for (i, c) in parity_crc.iter().enumerate() {
        out[parity_crc_off + 4 * i..parity_crc_off + 4 * i + 4].copy_from_slice(&c.to_le_bytes());
    }

    let header = geom.encode_header(crc32c::crc32c(&out[tables_off..shards_off]));
    out[..PARITY_HEADER_LEN].copy_from_slice(&header);
    Ok(out)
}

/// Bilan des dégâts d'une archive, à la granularité des shards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Damage {
    /// `None` si l'archive n'a pas de parité ou si la section est illisible.
    pub geometry: Option<Geometry>,
    /// Section de parité présente et auto-cohérente.
    pub parity_valid: bool,
    /// Shards de données effacés (CRC faux, ou repli sur un morceau au hachage faux).
    pub bad_data_shards: Vec<u64>,
    /// Slots de parité dont le CRC est faux.
    pub bad_parity_shards: Vec<u64>,
    /// Copies du trailer illisibles (0 à 2).
    pub bad_trailer_copies: u8,
    /// Vrai si tout dégât constaté peut être reconstruit.
    pub repairable: bool,
    /// Premier groupe dépassant son budget : (groupe, effacés, parité disponible).
    pub overflow: Option<(u64, u64, u32)>,
}

impl Damage {
    pub fn needs_repair(&self) -> bool {
        !self.bad_data_shards.is_empty()
            || !self.bad_parity_shards.is_empty()
            || self.bad_trailer_copies > 0
    }

    pub(crate) fn unrepairable_error(&self) -> Error {
        match self.overflow {
            Some((group, erased, max)) => Error::Unrepairable { group, erased, max },
            None => Error::NoParity,
        }
    }
}

/// Analyse la zone protégée et la section. `bad_chunk_ranges` : intervalles
/// d'octets des morceaux dont le hachage BLAKE3 ne correspond pas (repli).
pub fn scan(
    protected: &[u8],
    section: Option<&ParitySection<'_>>,
    bad_chunk_ranges: &[Range<u64>],
    bad_trailer_copies: u8,
) -> Damage {
    let Some(sec) = section else {
        return Damage {
            geometry: None,
            parity_valid: false,
            bad_data_shards: Vec::new(),
            bad_parity_shards: Vec::new(),
            bad_trailer_copies,
            repairable: bad_chunk_ranges.is_empty(),
            overflow: None,
        };
    };
    let geom = sec.geometry;
    let mut bad: BTreeSet<u64> = (0..geom.data_shards)
        .filter(|&i| crc32c::crc32c(&protected[geom.data_range(i)]) != sec.data_crc(i))
        .collect();
    for r in bad_chunk_ranges {
        let shards = geom.shards_covering(r.clone());
        if !shards.clone().any(|i| bad.contains(&i)) {
            bad.extend(shards);
        }
    }
    let bad_parity: Vec<u64> = (0..geom.parity_shards())
        .filter(|&slot| crc32c::crc32c(sec.parity_shard(slot)) != sec.parity_crc(slot))
        .collect();

    let mut erased = vec![0u64; geom.groups as usize];
    for &i in &bad {
        erased[geom.locate(i).0 as usize] += 1;
    }
    for &slot in &bad_parity {
        erased[geom.parity_slot_group(slot) as usize] += 1;
    }
    let overflow = erased
        .iter()
        .enumerate()
        .find(|(_, &e)| e > geom.recovery_per_group as u64)
        .map(|(g, &e)| (g as u64, e, geom.recovery_per_group));

    Damage {
        geometry: Some(geom),
        parity_valid: true,
        bad_data_shards: bad.into_iter().collect(),
        bad_parity_shards: bad_parity,
        bad_trailer_copies,
        repairable: overflow.is_none(),
        overflow,
    }
}

/// Octets à réécrire, à un offset absolu de l'archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub offset: u64,
    pub bytes: Vec<u8>,
}

/// Reconstruit les shards effacés (données puis parité) ; `section_off` est
/// l'offset absolu de la section dans l'archive. Ne modifie rien.
pub fn reconstruct(
    protected: &[u8],
    sec: &ParitySection<'_>,
    section_off: u64,
    damage: &Damage,
) -> Result<Vec<Patch>> {
    if !damage.repairable {
        return Err(damage.unrepairable_error());
    }
    let geom = sec.geometry;
    let m = geom.recovery_per_group as usize;
    let bad_data: BTreeSet<u64> = damage.bad_data_shards.iter().copied().collect();
    let bad_parity: BTreeSet<u64> = damage.bad_parity_shards.iter().copied().collect();
    let groups: BTreeSet<u64> = bad_data
        .iter()
        .map(|&i| geom.locate(i).0)
        .chain(bad_parity.iter().map(|&s| geom.parity_slot_group(s)))
        .collect();

    let mut patches = Vec::new();
    for g in groups {
        let k = geom.data_shards_in_group(g);
        let present: Vec<(usize, Cow<[u8]>)> = (0..k)
            .map(|pos| (pos, geom.shard_of_group(g, pos)))
            .filter(|(_, i)| !bad_data.contains(i))
            .map(|(pos, i)| (pos as usize, padded_shard(protected, &geom, i)))
            .collect();
        let recovery: Vec<(usize, &[u8])> = (0..m as u64)
            .map(|j| (j, geom.parity_slot(g, j)))
            .filter(|(_, slot)| !bad_parity.contains(slot))
            .map(|(j, slot)| (j as usize, sec.parity_shard(slot)))
            .collect();
        let restored = reed_solomon_simd::decode(
            k as usize,
            m,
            present.iter().map(|(p, b)| (*p, b.as_ref())),
            recovery,
        )
        .map_err(rs_err)?;
        for (pos, bytes) in &restored {
            let i = geom.shard_of_group(g, *pos as u64);
            let r = geom.data_range(i);
            patches.push(Patch {
                offset: r.start as u64,
                bytes: bytes[..r.len()].to_vec(),
            });
        }
        let group_parity_bad = (0..m as u64).any(|j| bad_parity.contains(&geom.parity_slot(g, j)));
        if group_parity_bad {
            let originals = (0..k).map(|pos| match restored.get(&(pos as usize)) {
                Some(b) => Cow::Borrowed(b.as_slice()),
                None => padded_shard(protected, &geom, geom.shard_of_group(g, pos)),
            });
            let fresh = reed_solomon_simd::encode(k as usize, m, originals).map_err(rs_err)?;
            for (j, bytes) in fresh.into_iter().enumerate() {
                let slot = geom.parity_slot(g, j as u64);
                if bad_parity.contains(&slot) {
                    patches.push(Patch {
                        offset: section_off + sec.parity_shard_offset(slot),
                        bytes,
                    });
                }
            }
        }
    }
    Ok(patches)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn protected(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 7 % 251) as u8).collect()
    }

    #[test]
    fn geometry_balances_interleaved_groups() {
        let opts = ParityOptions {
            percent: 5,
            shard_size: 64,
            group_max: 4,
        };
        let g = Geometry::new(64 * 10 + 3, &opts);
        assert_eq!(g.data_shards, 11);
        assert_eq!(g.groups, 3);
        assert_eq!(g.recovery_per_group, 2);
        let total: u64 = (0..g.groups).map(|x| g.data_shards_in_group(x)).sum();
        assert_eq!(total, 11);
        for i in 0..11 {
            let (grp, pos) = g.locate(i);
            assert_eq!(g.shard_of_group(grp, pos), i);
            assert!(pos < g.data_shards_in_group(grp));
        }
        assert_eq!(g.data_range(10).len(), 3);
        assert_eq!(g.shards_covering(60..130), 0..3);
    }

    #[test]
    fn slicing_is_exact() {
        // Plusieurs géométries : dernier groupe incomplet, dernier shard rempli,
        // petit groupe où aucune tranche n'est nécessaire.
        for (len, opts) in [
            (300_000usize, ParityOptions::default()),
            (
                1_234_567,
                ParityOptions {
                    percent: 7,
                    shard_size: 1024,
                    group_max: 300,
                },
            ),
            (
                70_000,
                ParityOptions {
                    percent: 50,
                    shard_size: 512,
                    group_max: 4,
                },
            ),
        ] {
            let data = protected(len);
            let geom = Geometry::new(len as u64, &opts);
            let (s, m) = (geom.shard_size as usize, geom.recovery_per_group as usize);
            let w = slice_width(s, geom.group_max as usize, m);
            let mut enc = ReedSolomonEncoder::new(geom.group_max as usize, m, s).unwrap();
            for g in 0..geom.groups {
                let whole = encode_slice(&mut enc, &data, &geom, g, 0, s, m).unwrap();
                for part in 0..s / w {
                    let sliced = encode_slice(&mut enc, &data, &geom, g, part, w, m).unwrap();
                    for j in 0..m {
                        let (a, b) = (j * s + part * w, j * w);
                        assert_eq!(&sliced[b..b + w], &whole[a..a + w], "g{g} p{part} j{j}");
                    }
                }
            }
        }
        assert_eq!(slice_width(4096, 1024, 103), 512);
        assert_eq!(slice_width(4096, 4, 2), 4096);
        assert_eq!(slice_width(96, 32768, 3277), 96);
    }

    #[test]
    fn section_roundtrip_and_repair() {
        let opts = ParityOptions {
            percent: 10,
            shard_size: 64,
            group_max: 8,
        };
        let data = protected(64 * 30 + 17);
        let section = build_section(&data, &opts).unwrap();
        assert_eq!(
            section,
            build_section(&data, &opts).unwrap(),
            "déterministe"
        );
        let sec = ParitySection::parse(&section, data.len() as u64).unwrap();
        let clean = scan(&data, Some(&sec), &[], 0);
        assert!(!clean.needs_repair() && clean.repairable);

        // Perte contiguë de 10 % + un slot de parité abîmé.
        let mut hurt = data.clone();
        let tenth = hurt.len() / 10;
        for b in &mut hurt[100..100 + tenth] {
            *b = 0;
        }
        let mut hurt_section = section.clone();
        let off = sec.parity_shard_offset(0) as usize;
        hurt_section[off] ^= 0xff;
        let sec2 = ParitySection::parse(&hurt_section, data.len() as u64).unwrap();
        let damage = scan(&hurt, Some(&sec2), &[], 0);
        assert!(damage.repairable, "{damage:?}");
        assert_eq!(damage.bad_parity_shards, vec![0]);
        let patches = reconstruct(&hurt, &sec2, data.len() as u64, &damage).unwrap();
        let mut whole = hurt.clone();
        whole.extend_from_slice(&hurt_section);
        for p in patches {
            whole[p.offset as usize..p.offset as usize + p.bytes.len()].copy_from_slice(&p.bytes);
        }
        assert_eq!(&whole[..data.len()], &data[..]);
        assert_eq!(&whole[data.len()..], &section[..]);
    }

    #[test]
    fn hash_fallback_marks_whole_chunk() {
        let opts = ParityOptions {
            percent: 20,
            shard_size: 64,
            group_max: 8,
        };
        let data = protected(64 * 8);
        let section = build_section(&data, &opts).unwrap();
        let sec = ParitySection::parse(&section, data.len() as u64).unwrap();
        let ranges: Vec<Range<u64>> = std::iter::once(70..200).collect();
        let damage = scan(&data, Some(&sec), &ranges, 0);
        assert_eq!(damage.bad_data_shards, vec![1, 2, 3]);
    }

    #[test]
    fn overflow_is_reported() {
        let opts = ParityOptions {
            percent: 5,
            shard_size: 64,
            group_max: 1024,
        };
        let data = protected(64 * 20);
        let section = build_section(&data, &opts).unwrap();
        let sec = ParitySection::parse(&section, data.len() as u64).unwrap();
        let mut hurt = data.clone();
        for b in &mut hurt[..64 * 5] {
            *b ^= 1;
        }
        let damage = scan(&hurt, Some(&sec), &[], 0);
        assert!(!damage.repairable);
        assert_eq!(damage.overflow, Some((0, 5, 2)));
        assert!(matches!(
            reconstruct(&hurt, &sec, data.len() as u64, &damage),
            Err(Error::Unrepairable { .. })
        ));
    }

    #[test]
    fn corrupted_section_is_rejected() {
        let data = protected(1000);
        let mut section = build_section(&data, &ParityOptions::default()).unwrap();
        section[PARITY_HEADER_LEN + 1] ^= 1;
        assert!(ParitySection::parse(&section, 1000).is_err());
        assert!(ParitySection::parse(&section, 999).is_err());
    }
}
