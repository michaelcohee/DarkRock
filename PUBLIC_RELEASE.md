# Public snapshot notes

This repository's working history is a private research record. Some earlier commits contain local machine paths. Publish the **clean snapshot** from `public-export/` as a new repository, not this private Git history.

The public snapshot contains code, generated-corpus tooling, and reports. It excludes the manufacturer's DWGs, the ODA-converted DXFs, generated corpora, local benchmark files, and the original local conversion manifest. The checked-in Tier B manifest is a path-redacted metadata copy. It contains filenames, sizes, and hashes, but no drawing bytes. Tier B results cannot be rerun without independently supplied source drawings and the pinned converter.

The README's opening scope is: **single volume, one M1, no network**. This limits all node-sprawl and routing claims to their measured local or modeled conditions.

The selected publication target is `michaelcohee/DarkRock`. Its initial public commit must be made from the clean snapshot directory; it will not expose the private commit history. No license file has been selected, so publication does not grant reuse rights beyond GitHub's normal viewing and forking terms.
