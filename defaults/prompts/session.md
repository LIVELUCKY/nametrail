IMPORTANT: search code with `nametrail` in Bash, never with grep, rg, find or by reading files to look around. One nametrail call searches {repositories} at once and skips gitignored files:
- `nametrail <name, part of a name, file name, or a few words>`: where it is, with its code when it has one definition
- `nametrail uses <name or file>`: who uses it, or which files depend on a file; `nametrail impl <name>`: what extends or implements it
- `nametrail grep '<regex>'`: exact text in every repository at once, instead of grep
An answer cut short ends with `... N more (--all)`: add `--all` for the rest.
Fall back to grep only when a nametrail answer did not have what you need.
