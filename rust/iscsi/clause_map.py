#!/usr/bin/env python3
"""Maps the compliance cases of a run onto the clauses of an SBC or SPC edition.

For each standard: the clauses that have "shall" statements, which of them some case cites, the share of
"shall" statements in cited clauses, and a score (passed / judged cases citing that standard). The output
holds clause numbers, counts and results only. The standards' own text is read from the licensed PDFs in
--standards and cached as text in --cache, outside the repository.

  clause_map.py --log run.log --standards /root --cache ~/.cache/spit-standards \
                --edition SPC-3=INCITS+408 --edition SBC-3=INCITS+514 > map.csv

Requires pypdf (and python3 with hashlib). The PDFs are encrypted with RC4/MD5; the workaround below uses
pypdf's pure-Python RC4 because this host's OpenSSL refuses the cryptography path.
"""

import argparse
import collections
import glob
import hashlib
import json
import os
import re
import sys

HEADING = re.compile(r"^(\d+(?:\.\d+)+)\s+([A-Z][^\n]{2,80})$")
CITED = re.compile(r"\b((?:SPC|SBC)-\d)\s+([0-9]+(?:\.[0-9]+)+)")
CASE = re.compile(r"^(PASS|FAIL|WARN|INFO)\s")


def extract(pdf, out):
    """Writes the text of an encrypted standard PDF to `out`, page by page."""
    import pypdf._crypt_providers._fallback as fallback
    import pypdf._encryption as encryption
    from pypdf import PdfReader

    real_md5 = hashlib.md5
    hashlib.md5 = lambda data=b"", **kw: real_md5(data, usedforsecurity=False)
    encryption.rc4_encrypt = fallback.rc4_encrypt
    encryption.rc4_decrypt = fallback.rc4_decrypt
    encryption.CryptRC4 = fallback.CryptRC4
    reader = PdfReader(pdf)
    if reader.is_encrypted:
        reader.decrypt("")
    with open(out, "w", encoding="utf-8") as f:
        for i, page in enumerate(reader.pages):
            f.write(f"\n=====PAGE {i + 1}\n")
            f.write(page.extract_text() or "")


def standard_text(standards, cache, edition):
    """The cached text of an edition, extracted from its PDF on first use."""
    os.makedirs(cache, exist_ok=True)
    out = os.path.join(cache, f"{edition}.txt")
    if not os.path.exists(out):
        matches = glob.glob(os.path.join(standards, f"{edition}*.pdf"))
        if not matches:
            sys.exit(f"no PDF for {edition} in {standards}")
        extract(matches[0], out)
    return out


def shall_per_clause(path):
    """Counts the "shall" statements under each numbered clause heading of the body."""
    counts = collections.OrderedDict()
    current = None
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.rstrip("\n")
            m = HEADING.match(line)
            if m and "...." not in line and not re.search(r"\s\d+$", line):
                current = m.group(1)
                counts.setdefault(current, 0)
                continue
            if current and re.search(r"\bshall\b", line, re.I):
                counts[current] += len(re.findall(r"\bshall\b", line, re.I))
    return counts


def cases_of(log_path):
    """Each judged case (PASS or FAIL) with the standards and clauses its label cites."""
    rows = []
    with open(log_path, encoding="utf-8", errors="replace") as f:
        for line in f:
            m = CASE.match(line)
            if not m or m.group(1) not in ("PASS", "FAIL"):
                continue
            cites = {(std, clause) for std, clause in CITED.findall(line)}
            rows.append((m.group(1), cites))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--log", required=True, help="the compliance log of the run (spit --compliance)")
    ap.add_argument("--standards", default="/root", help="folder with the licensed standard PDFs")
    ap.add_argument("--cache", default=os.path.expanduser("~/.cache/spit-standards"))
    ap.add_argument("--edition", action="append", required=True, metavar="STD=PDFPREFIX",
                    help="e.g. SBC-3=INCITS+514; repeat for each standard")
    args = ap.parse_args()

    cases = cases_of(args.log)
    print("standard,clause,shall_statements,cited,cases,passed,failed")
    summary = []
    for spec in args.edition:
        std, prefix = spec.split("=", 1)
        counts = shall_per_clause(standard_text(args.standards, args.cache, prefix))
        per = collections.defaultdict(lambda: [0, 0])
        judged = [r for r in cases if any(s == std for s, _ in r[1])]
        for result, cites in judged:
            for s, clause in cites:
                if s == std:
                    per[clause][0 if result == "PASS" else 1] += 1
        for clause, shall in counts.items():
            if shall == 0:
                continue
            p, f = per.get(clause, [0, 0])
            print(f"{std},{clause},{shall},{'yes' if p + f else 'no'},{p + f},{p},{f}")
        shall_total = sum(v for v in counts.values())
        cited_shall = sum(counts[c] for c in counts if counts[c] and (std, c) in
                          {(s, cl) for _, cs in cases for s, cl in cs})
        clauses = [c for c, v in counts.items() if v]
        cited = [c for c in clauses if per.get(c, [0, 0]) != [0, 0]]
        passed = sum(1 for result, cs in judged if result == "PASS")
        failed = sum(1 for result, cs in judged if result == "FAIL")
        score = 100.0 * passed / (passed + failed) if passed + failed else float("nan")
        summary.append({
            "standard": std,
            "clauses_with_shall": len(clauses),
            "clauses_cited": len(cited),
            "clause_coverage_pct": round(100.0 * len(cited) / max(1, len(clauses)), 1),
            "shall_total": shall_total,
            "shall_in_cited": cited_shall,
            "shall_coverage_pct": round(100.0 * cited_shall / max(1, shall_total), 1),
            "cases_judged": passed + failed,
            "score_pct": round(score, 1) if score == score else None,
        })
    print(json.dumps({"summary": summary}, indent=2), file=sys.stderr)


if __name__ == "__main__":
    main()
