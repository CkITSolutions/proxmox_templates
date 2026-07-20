# Release v1.1.2

## Title

v1.1.2 — CentOS downloads, cluster VMIDs, and root password SSH

## Short description

Fixes CentOS Stream 403 downloads, enables root/password SSH on all templates, and auto-offsets VMIDs on multi-node clusters.

## Release notes

### Highlights

- **CentOS Stream installs fixed** — resolve dated GenericCloud images instead of the `-latest` symlink that often returns HTTP 403; continue installing other templates if one fails
- **Root + password SSH** — vendor-data sets `PermitRootLogin yes`, `PasswordAuthentication yes`, and `disable_root: false` (overrides `prohibit-password`)
- **Cluster-safe VMIDs** — if a base ID is taken on another node, use `+100` / `+200` (e.g. Ubuntu 26.04: `1002` → `1102` → `1202`)
- **Host CPU** — templates use `--cpu host` for modern RHEL-family images

### Upgrade notes

1. Download `downloader_x86` and `x86_checksum.txt` from this release
2. Run the downloader once to refresh `/var/lib/vz/snippets/ckits-vendor-data.yaml`
3. Reinstall templates (replace local VMIDs) so new clones get the SSH and CPU settings

### Checksum

```
02a39f5957087d1f243be267c2d82421e24b29eee8737eab316632aaa8bd31cd  downloader_x86
```

Also attached as `x86_checksum.txt` on this release.
