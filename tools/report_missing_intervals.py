#!/usr/bin/env python3
"""Read-only, fail-closed UTC report for VSEPH sparse-span staging files.

The report is an investigation queue, NOT a certified coverage manifest.
"""

import argparse
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import sys


EPOCH = datetime(1970, 1, 1, tzinfo=timezone.utc)
KINDS = {
    "between_recorded_anchors": "lacuna tra ancore registrate",
    "leading_unconfirmed_boundary": "limite iniziale non confermato",
    "trailing_unconfirmed_boundary": "limite finale non confermato",
    "no_recorded_anchors_in_window": "nessuna ancora nella finestra",
}


def integer(value, name):
    if type(value) is not int:  # bool is not a timestamp
        raise ValueError(f"{name}: expected integer")
    return value


def instant(us):
    """Format an integer POSIX microsecond instant without float rounding."""
    try:
        return (EPOCH + timedelta(microseconds=us)).isoformat(timespec="microseconds").replace(
            "+00:00", "Z"
        )
    except (OverflowError, ValueError):
        return f"{us} UTC/POSIX us (outside ISO calendar range)"


def read_stage(path):
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(data, dict) or data.get("kind") != "unverified_sparse_interval_staging":
        raise ValueError(f"{path}: not a VSEPH sparse-interval staging object")
    if data.get("coverage_certified") is not False or data.get("generation_complete") is not False:
        raise ValueError(f"{path}: certification/completion flags must be false")
    if data.get("mode") not in ("plan", "calculate"):
        raise ValueError(f"{path}: invalid mode")
    if data.get("family") not in ("synods", "motions", "ingresses"):
        raise ValueError(f"{path}: invalid family")
    if not isinstance(data.get("body"), str) or not data["body"]:
        raise ValueError(f"{path}: invalid body")
    dataset = data.get("dataset")
    digest = dataset.get("sha256") if isinstance(dataset, dict) else None
    if not isinstance(digest, str) or len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        raise ValueError(f"{path}: missing canonical dataset SHA-256")
    window = data.get("requested_window")
    if not isinstance(window, dict):
        raise ValueError(f"{path}: missing requested window")
    start = integer(window.get("start_us"), "window start")
    end = integer(window.get("end_us"), "window end")
    threshold = integer(data.get("sparse_threshold_us"), "threshold")
    if end <= start or threshold <= 0:
        raise ValueError(f"{path}: invalid window or threshold")
    spans, candidates = data.get("sparse_spans"), data.get("candidates")
    if not isinstance(spans, list) or not isinstance(candidates, list):
        raise ValueError(f"{path}: missing spans/candidates")
    if data["mode"] == "plan" and candidates:
        raise ValueError(f"{path}: plan mode cannot contain calculated candidates")
    parsed = []
    previous_end = start
    for i, span in enumerate(spans):
        if not isinstance(span, dict) or span.get("kind") not in KINDS:
            raise ValueError(f"{path}: invalid sparse span #{i}")
        a = integer(span.get("start_us"), f"span #{i} start")
        b = integer(span.get("end_us"), f"span #{i} end")
        if not (start <= a < b <= end) or a < previous_end or b - a <= threshold:
            raise ValueError(f"{path}: unsorted/overlapping/out-of-window/too-short span #{i}")
        previous_end = b
        for k in ("previous_original_source_id", "next_original_source_id"):
            if span.get(k) is not None and (type(span[k]) is not int or span[k] < 0):
                raise ValueError(f"{path}: invalid {k}")
        for k in ("previous_recorded_us", "next_recorded_us"):
            if span.get(k) is not None:
                integer(span[k], k)
        parsed.append({
            "start_us": a,
            "end_us": b,
            "start_utc": instant(a),
            "end_utc_exclusive": instant(b),
            "kind": span["kind"],
            "description": KINDS[span["kind"]],
            "status": "da_calcolare" if data["mode"] == "plan" else "candidati_non_convalidati",
            "previous_source_id": span.get("previous_original_source_id"),
            "next_source_id": span.get("next_original_source_id"),
            "source": str(path),
        })
    return {"sha256": digest, "family": data["family"], "body": data["body"],
            "threshold_us": threshold, "start": start, "end": end,
            "mode": data["mode"], "spans": parsed, "candidate_count": len(candidates), "source": str(path)}


def aggregate(paths):
    if not paths:
        raise ValueError("at least one staging JSON path is required")
    stages = [read_stage(path) for path in paths]
    first = stages[0]
    for stage in stages[1:]:
        for field in ("sha256", "family", "body", "threshold_us"):
            if stage[field] != first[field]:
                raise ValueError(f"incompatible stages: {field} differs; report each separately")
    stages.sort(key=lambda s: (s["start"], s["end"]))
    unchecked = []
    for left, right in zip(stages, stages[1:]):
        if left["end"] > right["start"]:
            raise ValueError("overlapping audit windows: report separately to avoid duplicate claims")
        if left["end"] < right["start"]:
            unchecked.append({"start_utc": instant(left["end"]),
                              "end_utc_exclusive": instant(right["start"])})
    return {
        "kind": "vseph-unverified-sparse-interval-report-v1",
        "coverage_certified": False,
        "scientific_completeness_established": False,
        "dataset_sha256": first["sha256"],
        "family": first["family"], "body": first["body"],
        "threshold_us": first["threshold_us"],
        "audited_windows": [{"start_utc": instant(s["start"]),
                              "end_utc_exclusive": instant(s["end"]),
                              "mode": s["mode"], "candidate_count": s["candidate_count"],
                              "source": s["source"]} for s in stages],
        "flagged_intervals": sorted((span for s in stages for span in s["spans"]), key=lambda x: x["start_us"]),
        "unchecked_between_windows": unchecked,
        "outside_audited_windows": "unknown: not evaluated",
    }


def markdown(data):
    rows = ["# VSEPH — intervalli da esaminare (NON certificati)", "",
            f"Famiglia: **{data['family']}** · corpo: **{data['body']}** · SHA-256 sorgente: `{data['dataset_sha256']}`",
            f"Soglia scelta: **{data['threshold_us']} microsecondi** (euristica, non garanzia astronomica).",
            "Tutti gli intervalli sono in UTC, con inizio incluso e fine **esclusa** `[inizio, fine)`.",
            "", "## Intervalli segnalati", "",
            "| Dal (UTC) | Al (UTC, escluso) | Motivo | Stato | ID ancore preced./succ. |",
            "| --- | --- | --- | --- | --- |"]
    for span in data["flagged_intervals"]:
        anchor = " / ".join(str(span[k]) if span[k] is not None else "—"
                            for k in ("previous_source_id", "next_source_id"))
        rows.append(f"| {span['start_utc']} | {span['end_utc_exclusive']} | {span['description']} | {span['status']} | {anchor} |")
    if not data["flagged_intervals"]:
        rows.append("| — | — | Nessun intervallo **segnalato dalla soglia**; copertura NON dimostrata | non_convalidata | — |")
    rows += ["", "## Finestre esaminate", "",
             "| Dal (UTC) | Al (UTC, escluso) | Esecuzione | Candidati non verificati |",
             "| --- | --- | --- | ---: |"]
    for window in data["audited_windows"]:
        rows.append(f"| {window['start_utc']} | {window['end_utc_exclusive']} | {window['mode']} | {window['candidate_count']} |")
    rows += ["", "## Intervalli non esaminati fra le finestre fornite", ""]
    if data["unchecked_between_windows"]:
        rows += ["| Dal (UTC) | Al (UTC, escluso) | Stato |", "| --- | --- | --- |"]
        for interval in data["unchecked_between_windows"]:
            rows.append(f"| {interval['start_utc']} | {interval['end_utc_exclusive']} | non_verificato |")
    else:
        rows.append("Nessuna discontinuità **fra le finestre fornite**; questo non certifica i dati.")
    rows += ["", "**Fuori da tutte le finestre esaminate:** stato sconosciuto (non controllato).",
             "**Dentro le finestre ma sotto la soglia:** copertura NON certificata; il rilevatore può non vedere eventi mancanti.",
             "**Nessun risultato autorizza importazione o pubblicazione** senza confronto astronomico indipendente e approvazione.", ""]
    return "\n".join(rows)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("staging", nargs="+", help="one or more vseph-fill-gaps staging JSON files")
    parser.add_argument("--format", choices=("markdown", "json"), default="markdown")
    args = parser.parse_args(argv)
    try:
        report = aggregate(args.staging)
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as exc:
        parser.error(str(exc))
    print(json.dumps(report, indent=2, ensure_ascii=False) if args.format == "json" else markdown(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
