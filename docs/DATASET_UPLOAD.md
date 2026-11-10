# How the 2026-11-10 snapshot is stored on GitHub

The author requested publication of all three scientific CSV tables and the experimental compressed VSEPH-P0 dataset in the public repository. Their **actual data blobs** have been attached to [draft PR #6](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/pull/6), but remain unmerged pending review. The large-file GitHub integration cannot upload a ~4–12 MB file in one request, so it stores some files **byte-for-byte across ordered binary blobs**, with a verified reassembly tool.

## On-disk Git layout

Each of the following may be stored as one normal GitHub file, or as binary parts under `<name>.parts/part-NNN`:

- `datasets/2026-11-10/sinodi.csv`
- `datasets/2026-11-10/moto_pianeti.csv`
- `datasets/2026-11-10/ingresso_segni.csv`
- `datasets/2026-11-10/yearly_counts.csv`
- `datasets/2026-11-10/virgostars-events-20261110.vseph-p0`

The main CSV tables have **headers and normalized UTC ISO timestamps**, not the original pgAdmin4 no-header raw layout. Source IDs, signs, motion directions and all sinodic phases are preserved. Application fields `interpretazione` and `messaggio_id` are omitted.

## Reassemble and validate

After cloning this branch, with **Python 3.10+**, from the repository root:

```bash
python tools/assemble_dataset.py
python -m unittest test/test_assemble_dataset.py
python prototype/test_vseph_prototype.py
python prototype/virgostars_vseph_reader.py datasets/2026-11-10/_assembled/virgostars-events-20261110.vseph-p0 synod Mercury 2026-11-10T12:00:00Z
```

The first command writes complete files under `datasets/2026-11-10/_assembled/` (intentionally ignored by Git) and verifies each exact expected **byte length**, **SHA-256**, CSV **header** and **row count**; binary magic checked separately. A corrupted or missing part fails rather than silently omitting records. The `_assembled` directory can be deleted/recreated any time.

To verify outputs directly using Python, `hashlib.file_digest` or `sha256sum` must match each file listed in `manifest-publication.json`.

To inspect potential missing intervals, read `yearly_counts.csv` and `candidate_large_intervals.csv`. A zero-count year or long interval is a **candidate for investigation**, not proof that a physical event happened or was missing. Add scientifically supported events to a separate version, never silently edit the immutable original snapshot.

## Windows (PowerShell)

```powershell
py -3 .\tools\assemble_dataset.py
py -3 -m unittest test\test_assemble_dataset.py
Get-FileHash .\datasets\2026-11-10\_assembled\virgostars-events-20261110.vseph-p0 -Algorithm SHA256
```

Expected compressed binary SHA-256:

`c7746927f9132aea8fc338524d4bc6927010229e1589ff6f1bff5e4f8f6d8c2e`

## Data licensing / collaboration

The dataset is original work **according to the owner**, who requested public repository publication. The source code is CPAL-1.0 with VirgoStars credits. A **separate open-data license has not yet been selected** for the scientific snapshot. Do not claim open-data licensing or unrestricted redistribution merely because the dataset is public. Future contributors should clarify ownership/provenance for additions and request maintainer review.

## Format stability

`VSEPH-P0` is an **experimental, non-final Python prototype**. Do not confuse it with the future Rust **VSEPH v1** format. The compressed file stores precisely the original events; it does not calculate absent dates. The Rust engine will use a distinct documented, versioned file layout as decided in issue #3. Tests of byte-for-byte reconstruction do not validate astronomical correctness.
