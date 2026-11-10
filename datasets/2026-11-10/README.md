# VirgoStars event snapshot — 2026-11-10

**Observed data are incomplete. No astronomical event is to be fabricated or silently interpolated.**

The dataset owner supplied the source PostgreSQL exports and explicitly requested public repository publication. The **dataset reuse license remains undecided**; code is CPAL-1.0, which does not automatically license the source data. Do not invent extra permissions. This snapshot is a historical reference and a starting point for additions by contributors, not a complete solar-system ephemeris.

## Inventory

| Family | PostgreSQL source | Observed records |
| --- | --- | ---: |
| Synod cycles | `sinodi` | 70,554 |
| Direct/retrograde stations | `moto_pianeti` | 266,516 |
| Sign ingresses | `ingresso_segni` | 43,774 |
| **Total** | | **380,844** |

Files in the publication-ready package include a **VSEPH-P0 experimental zlib-compressed** archive and three scientific CSV projections, plus an annual event-count index. The actual archive and CSV data blobs have been committed on [draft PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6); they are **not yet merged to main**. Read the multipart storage instructions below.

## Canonical paths

When the files have been uploaded, expect:

- `sinodi.csv` — `id,elemento,dataora_utc,dataora_extra_utc_json`. Phase extras are a JSON array or empty field for SQL NULL.
- `moto_pianeti.csv` — `id,moto,segno,dataora_utc,elemento_celeste`.
- `ingresso_segni.csv` — `id,segno,dataora_utc,elemento_celeste`.
- `virgostars-events-20261110.vseph-p0` — **candidate experimental** indexed compressed binary (not final Rust VSEPH v1).
- `yearly_counts.csv` — each observed count by family/body/calendar year (0 is not scientific proof of a missing event).
- `candidate_large_intervals.csv` — heuristic suspicious long inter-event intervals, not scientifically verified missing calculations.
- `manifest-publication.json` — exact SHA-256 per file and canonical scientific event digests.

The CSVs deliberately exclude the `interpretazione` and `messaggio_id` application fields (9 and 332 nonempty cells, respectively) to avoid putting Telegram-linked or interpretive material in a scientific repository. Original database row IDs and all astronomical fields are preserved. The normalized CSVs match the compressed archive's records exactly by independent canonical SHA-256 comparison.

## Coverage and known gaps

- Synod events: combined min/max years **0002–2999**; motion: **0100–2999**. This is not proof that every body or year was calculated.
- Most sign ingress families: about **1900–2099**, often much shorter for slow outer planets.
- Moon sign ingress has a **multi-century gap** from the end of 2099 until 4836, followed by 2,181 Aries-only records until 4999. These must **never** be used to claim the Moon was in Aries throughout the gap.
- Two unexpectedly long Moon synod adjacent-record intervals also require verification: 1918–1920 and 2017–2019. See `candidate_large_intervals.csv`.
- `sinodi` used PostgreSQL TIMESTAMPTZ; motion/ingress source timestamps were stored without a time zone and have been interpreted as UTC according to application convention. Confirm this interpretation before publication of scientific conclusions.

## Contributor workflow

Choose one family/body/year, inspect coverage and event-level gaps, document your calculation method and reference/version, generate candidate events in a separate patch, and provide independent tests. **Never** fill a gap by copying the previous event state or extrapolating a sign. New events require provenance, checksums, schema validation, duplicate detection and reviewer approval. Keep original snapshot files immutable; place incremental additions in new dated datasets or versioned patches.

## Reproduction

The original P0 packer/reader is a Python proof of concept (not the Rust crate, and not VSEPH v1). Published CSVs are **normalized**; its original `pack`/`verify` CLI expects older pgAdmin-shaped exports and cannot consume these new headered CSVs without an explicit adapter. The manifest records cryptographic checksums for all scientific fields, including source IDs, and may be used for independent parity validation.

Verify any files you receive by their exact SHA-256 before treating them as the source snapshot. No GitHub Actions or automated release is enabled.

## Getting the large files into GitHub

The programmatic GitHub interface used for initialization can reject multi-megabyte requests. If the dataset is stored as `*.parts/part-NNN`, use the dedicated assembler described in [docs/DATASET_UPLOAD.md](../../docs/DATASET_UPLOAD.md) (after it is committed), verify checksum, then inspect normal CSVs. Git history should preserve immutable content, not imply all bodies have continuous scientific coverage.

## Tracking work on missing intervals

- [Issue #7](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/7) — two unusually long Moon synod intervals (candidate gaps, independent re-calculation needed).
- [Issue #8](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/8) — Moon ingress discontinuity 2099–4836 and isolated Aries-only tail; ensure unknown coverage is surfaced to clients.
- [Issue #9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9) — select a separate open-data reuse license; public repository visibility alone does not grant unrestricted reuse rights.
