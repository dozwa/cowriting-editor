# Programs the editor brings along

The export to PDF and Word is made by two programs that ship inside the installer, unchanged,
as separate programs the editor starts:

| Program | Version | Licence | Source |
|---|---|---|---|
| [pandoc](https://pandoc.org) | 3.14 | GPL-2.0-or-later, © John MacFarlane and others | https://github.com/jgm/pandoc/tree/3.14 |
| [Typst](https://typst.app) | 0.15.1 | Apache-2.0 | https://github.com/typst/typst/tree/v0.15.1 |

The licence texts are in `licenses/`. The binaries are the projects' own release files; their
source code is at the links above. The CoWriting Editor is not derived from either program.

The two versions belong together: pandoc writes Typst markup for the Typst version of its time,
and an older pandoc with a newer Typst fails or loses the links in the reference list.

The binaries are not in the repository. `python3 scripts/fetch-tools.py` downloads them for
this system into this folder and checks them against the pinned SHA-256 sums.
