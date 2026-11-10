# Verify all public CSV records against VSEPH-P0

**Experimental development check, not a stable v1 API or astronomy-calculation engine.** The code is on [draft PR #5](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/5); the normalized 2026-11-10 scientific CSVs and VSEPH-P0 archive are on the separate [draft dataset PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6). Neither PR needs to be merged to `main` just to perform a local review.

## Why this additional audit matters

Checking a file's SHA-256 and 761 compressed-block checksums proves that bytes match an expected snapshot, **not that CSV exports and binary contain identical scientific fields**. This independent tool compares every corresponding record, including historic years far before 1970:

- Original PostgreSQL row ID and celestial body (including exact spelling);
- Exact signed UTC/POSIX microsecond timestamp, without floating-point rounding;
- Synod phase arrays, including the important distinction **SQL NULL vs JSON []** and the exact order/time of any additional phases;
- Zodiac sign (0–11), and motion direction (`r` retrograde / `d` direct);
- Presence of every record in **both** representations, with duplicate-source-ID rejection.

The public normalized CSVs must have the exact headers from PR #6:
`sinodi.csv`: `id,elemento,dataora_utc,dataora_extra_utc_json`;
`moto_pianeti.csv`: `id,moto,segno,dataora_utc,elemento_celeste`;
`ingresso_segni.csv`: `id,segno,dataora_utc,elemento_celeste`.

The tool's input is the **public normalized CSVs with headers** (not the earlier raw pgAdmin4 exports). It deliberately excludes Telegram metadata and interpretive texts. It does not edit the inputs or source PostgreSQL; it uses a disposable local SQLite database for the exact per-ID comparison, so individual mismatches produce the family/body/source-ID context and cannot disappear into a row-count summary. Python 3.10+ standard library only; **Rust/Cargo is not required** for this comparison.

## Run on the 2026-11-10 snapshot

Create a temporary local checkout containing the code from draft PR #5 and the dataset from draft PR #6, following [the dataset assembly guide](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6). After `python tools/assemble_dataset.py` reconstructs the archive:

```sh
python tools/verify_csv_p0.py datasets/2026-11-10 datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 --sha256 c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e
```

On Windows PowerShell, replace `python` with `py -3` if needed; the same paths work from repo root. This command uses the independently published binary SHA-256 for this **exact immutable version**. For future datasets, use their own manifest SHA-256, not the value above.

Expected JSON includes:
```json
{
  "result": "exact_scientific_field_parity",
  "records_compared": 380844,
  "sections": { "synod": 70554, "motion": 266516, "ingress": 43774 },
  "blocks_checked": 761,
  "archive_sha256_checked": true,
  "coverage_certified": false
}
```

It also reports the SHA-256 for each normalized CSV file and the complete binary. Any missing, duplicated, added or field-altered record causes a nonzero exit.

## Run synthetic regression tests

```sh
python -m unittest discover -s tools -p test_verify_csv_p0.py -v
```

The 11 synthetic tests deliberately exercise exact parity, negative and year-0002 timestamps, modified ingress sign, changed station direction, loss of the NULL/empty phase distinction, changed synod phase time, missing and duplicate rows, a damaged archive, mismatched expected hash and an invalid CSV header.

### Verified in the authoring environment

- **11/11 synthetic tests passed.** The executed scripts match their committed GitHub blob SHA-1 exactly. The local integration harness uses a compatible `audit_file` function; running the combined draft checkout with the repository's exact independent auditor remains a required follow-up.
- **380,844/380,844 individual rows matched** when comparing the published normalized CSV bytes from the owner-authorized 2026-11-10 publication bundle against the original 4,143,385-byte P0 archive. Counts: **70,554 synods; 266,516 motion stations; 43,774 ingress records**. No scientific-field mismatches were found.
- The archive SHA-256 matched `c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e`. The independent Python byte auditor is already provided by PR #5; Rust compilation and Rust/Python runtime parity remain **NOT RUN** due to the absent Cargo toolchain.

**Important scientific boundary:** exact CSV ↔ P0 equivalence is a data-integrity result. It does **not** prove any astronomical value is correct or that the periods missing in the source tables have been calculated. Those are separate review tasks (issues #7 and #8). Do not silently fill gaps, alter timestamps or declare coverage without independently documented evidence.

Attribution: **Powered by VirgoStars** — https://www.virgostars.it/. Code CPAL-1.0; the dataset's reuse license is tracked separately in [issue #9](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/9).
