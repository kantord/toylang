#!/usr/bin/env python3
"""Fetches this machine's own copies of the Project Euler puzzle data that tests/euler_real_data.rs
verifies against, into a local cache -- never into the repo (kantord/toylang#39: the data is
problem-specific content, not narrative, and stays out of version control either way).

Source and licence: Project Euler's main problem content, including the numbers and grids this
pulls, is Creative Commons BY-NC-SA 4.0 (https://projecteuler.net/copyright). This script fetches
five pages a normal solver would read anyway (https://projecteuler.net/minimal={8,11,13,18} and
the published names file), for this one machine's own non-commercial verification run, and writes
plain-text copies to a gitignored local directory -- nothing is redistributed, published, or
committed. A SOURCES.txt next to the data records where each file came from and under what
licence, satisfying the attribution term without polluting the data files the strict test parsers
read (a comment line in e.g. euler13.txt would misparse as "not a digit").

Usage: python3 scripts/fetch_euler_data.py [DEST_DIR]   (default: .euler-data, repo-relative)
Re-run any time; a file already present is left alone unless --force is given.
"""

from __future__ import annotations

import html
import re
import sys
import time
import urllib.request
from pathlib import Path

USER_AGENT = "toylang-euler-data-fetch/1 (+https://github.com/kantord/toylang, non-commercial verification run)"
COPYRIGHT_NOTE = (
    "Project Euler main problem content is CC BY-NC-SA 4.0 -- https://projecteuler.net/copyright"
)

# (problem, published answer -- checked by tests/euler_real_data.rs, not by this script)
MINIMAL_PROBLEMS = ["8", "11", "13", "18"]
NAMES_URL = "https://projecteuler.net/resources/documents/0022_names.txt"

# The block Project Euler itself marks as the copy-paste data, a <p> or <div> tagged
# "copy_to_clipboard" -- the one discriminator that survives a problem having other monospace
# blocks too (problem 18's own worked example is also class="monospace", without this class).
BLOCK_RE = re.compile(
    r'<(p|div)\s+class="[^"]*\bcopy_to_clipboard\b[^"]*"[^>]*>(.*?)</\1>',
    re.DOTALL,
)
BR_RE = re.compile(r"<br\s*/?>", re.IGNORECASE)
TAG_RE = re.compile(r"<[^>]+>")


def fetch(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=30) as resp:
        return resp.read().decode("utf-8")


def extract_block(page_html: str, problem: str) -> str:
    m = BLOCK_RE.search(page_html)
    if not m:
        raise SystemExit(
            f"euler{problem}: no copy_to_clipboard block in the fetched page -- "
            "projecteuler.net's markup may have changed; update BLOCK_RE"
        )
    body = BR_RE.sub("\n", m.group(2))
    body = TAG_RE.sub("", body)
    body = html.unescape(body)
    # Each source line may carry its own leading/trailing whitespace from the markup's own
    # indentation; the Rust parsers split on lines and then on runs of whitespace, so this only
    # tidies the file for a human reader and changes nothing the parsers see.
    lines = [line.strip() for line in body.strip("\n").split("\n")]
    return "\n".join(line for line in lines if line) + "\n"


def write_if_absent(path: Path, text: str, source: str, force: bool) -> bool:
    if path.exists() and not force:
        print(f"  {path.name}: already cached, skipping (--force to refetch)")
        return False
    path.write_text(text)
    print(f"  {path.name}: wrote {len(text)} bytes from {source}")
    return True


def main() -> None:
    args = [a for a in sys.argv[1:] if a != "--force"]
    force = "--force" in sys.argv[1:]
    dest = Path(args[0]) if args else Path(".euler-data")
    dest.mkdir(parents=True, exist_ok=True)

    print(f"Fetching Project Euler data into {dest}/ ({COPYRIGHT_NOTE})")
    sources: list[str] = []
    fetched_any = False

    for problem in MINIMAL_PROBLEMS:
        out = dest / f"euler{int(problem):02d}.txt"
        url = f"https://projecteuler.net/minimal={problem}"
        if out.exists() and not force:
            write_if_absent(out, "", url, force)
            sources.append(f"euler{int(problem):02d}.txt  <- {url}")
            continue
        page = fetch(url)
        text = extract_block(page, problem)
        wrote = write_if_absent(out, text, url, force)
        sources.append(f"euler{int(problem):02d}.txt  <- {url}")
        if wrote:
            fetched_any = True
            time.sleep(1)  # a few requests to one server; no reason to rush them

    names_out = dest / "euler22.txt"
    if names_out.exists() and not force:
        write_if_absent(names_out, "", NAMES_URL, force)
    else:
        text = fetch(NAMES_URL)
        write_if_absent(names_out, text, NAMES_URL, force)
        fetched_any = True
    sources.append(f"euler22.txt      <- {NAMES_URL}")

    sources_note = dest / "SOURCES.txt"
    sources_note.write_text(
        "Fetched by scripts/fetch_euler_data.py for local, non-commercial verification only.\n"
        f"{COPYRIGHT_NOTE}\n"
        "Not part of the toylang repository; this directory is gitignored.\n\n"
        + "\n".join(sources)
        + "\n"
    )

    if fetched_any:
        print(f"Done. Run `just euler-data {dest}` (or bare `just euler-data`, if {dest} is the default).")
    else:
        print("Nothing new to fetch; all five files were already cached.")


if __name__ == "__main__":
    main()
