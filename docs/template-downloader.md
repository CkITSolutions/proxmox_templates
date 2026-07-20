# Template Downloader

Use the **Ck IT Solutions Template Downloader** to install cloud-init enabled VM templates on your Proxmox node.

**Current release:** v1.1.2

## Download the script

Download the latest release binary onto your Proxmox node (via SSH or the Proxmox web terminal):

```sh
wget https://github.com/CkITSolutions/proxmox_templates/releases/latest/download/downloader_x86
```

Verify the checksum (optional):

```sh
wget https://github.com/CkITSolutions/proxmox_templates/releases/latest/download/x86_checksum.txt
sha256sum -c x86_checksum.txt
```

Make the binary executable and run it:

```sh
chmod +x downloader_x86
./downloader_x86
```

## Interactive mode

When you run `./downloader_x86` without extra flags, the tool walks you through each step.

### Step 1: Select templates

A checklist appears with every supported operating system. Use the keyboard to choose what you want:

- **Arrow keys** — move up and down
- **Space** — select or deselect a template
- **A** — select all templates
- **Enter** — confirm your selection

Example entries:

```
[ ] Ubuntu 22.04 (Ubuntu)
[ ] Ubuntu 24.04 (Ubuntu)
[ ] Debian 12 (Debian)
[ ] CentOS Stream 9 (CentOS)
[ ] Fedora 44 (Fedora)
```

You can install one template or many at once. If one template fails, the others still continue.

### Step 2: Choose storage

If more than one storage pool is available, pick the pool where VM disks should be stored. The default is usually `local-lvm`.

Make sure the selected storage is configured to store **Disk image** content in Proxmox.

### Step 3: Resolve VMIDs (replace or cluster offset)

Proxmox requires unique VMIDs across the whole cluster. The downloader handles that as follows:

- If the base VMID is free → use it (for example `1002` for Ubuntu 26.04).
- If a VMID in that template’s series already exists **on this node** → you are asked to replace it (or it is replaced automatically with `--yes`).
- If the base VMID exists **on another cluster node** (or you decline replace) → the next free offset is used: `+100`, `+200`, …

Examples for Ubuntu 26.04 (`1002`):

| Node | VMID |
| --- | --- |
| First node | `1002` |
| Second node | `1102` |
| Third node | `1202` |

Offsets stay in the same thousand band (for example `1002` … `1902`) so they do not collide with other template families.

### Step 4: Download and install

The tool downloads the latest official cloud image for each selected template, imports it into Proxmox, configures cloud-init, and converts the VM into a template.

Progress is shown for each template. When finished, the templates are ready to use in Proxmox.

## Non-interactive mode

For automation or scripting, pass flags instead of using the menus:

```sh
./downloader_x86 --non-interactive --templates 1001,4001,6002 --storage local-lvm --yes
```

| Flag | Description |
| --- | --- |
| `--templates`, `-t` | Comma-separated list of **base** VMIDs to install |
| `--all` | Install every supported template |
| `--storage`, `-s` | Storage pool for VM disks |
| `--yes`, `-y` | Replace existing **local** VMIDs without prompting |
| `--non-interactive` | Skip all interactive prompts |

### Examples

Install Ubuntu 24.04 and Debian 12:

```sh
./downloader_x86 --non-interactive --templates 1001,4001 --storage local-lvm --yes
```

Install all templates:

```sh
./downloader_x86 --non-interactive --all --storage local-lvm --yes
```

## Always fetching the latest images

Templates are built from official upstream sources. On every run, the tool fetches the current image:

| Source | How latest is resolved |
| --- | --- |
| Ubuntu | Official release cloud image (updated in place) |
| Debian | `/latest/` image path on `cloud.debian.org` |
| Fedora | Newest `Fedora-Cloud-Base-Generic` image from the official release directory |
| CentOS Stream | Newest **dated** `CentOS-Stream-GenericCloud-*` image from `cloud.centos.org` (avoids broken `-latest` symlinks that can return HTTP 403) |
| Rocky Linux | `.latest` image symlink on `download.rockylinux.org` |
| Alma Linux | `-latest` image symlink on `repo.almalinux.org` |
| openSUSE Leap | Official `-Cloud.qcow2` image symlink |

Re-running the downloader always pulls a fresh image before building the template.

## Cloud-init and SSH access

Every template is created with:

- A cloud-init drive (`ide2`)
- DHCP networking
- Host CPU type (required for modern RHEL-family images such as CentOS Stream, Rocky, and Alma)
- QEMU guest agent pre-installed via a vendor-data snippet
- SSH key **and** password authentication enabled
- `PermitRootLogin yes` (overrides the common cloud-image default `prohibit-password`)
- Serial console for reliable access

Set a CI user/password and/or SSH key in Proxmox when cloning. Root password login works alongside key-based login.

The cloud-init snippet is stored at `/var/lib/vz/snippets/ckits-vendor-data.yaml` and is refreshed automatically when you run a newer downloader build. Reinstall or re-clone templates after upgrading so guests pick up SSH changes on first boot.

## VMID reference

Base VMIDs (first cluster node). Additional nodes use `base + 100`, `base + 200`, and so on.

| Base VMID | Template | Node 2 | Node 3 |
| --- | --- | --- | --- |
| 1000 | Ubuntu 22.04 | 1100 | 1200 |
| 1001 | Ubuntu 24.04 | 1101 | 1201 |
| 1002 | Ubuntu 26.04 | 1102 | 1202 |
| 3000 | CentOS Stream 9 | 3100 | 3200 |
| 3001 | CentOS Stream 10 | 3101 | 3201 |
| 4000 | Debian 11 | 4100 | 4200 |
| 4001 | Debian 12 | 4101 | 4201 |
| 4002 | Debian 13 | 4102 | 4202 |
| 5000 | Rocky Linux 8 | 5100 | 5200 |
| 5001 | Rocky Linux 9 | 5101 | 5201 |
| 5002 | Rocky Linux 10 | 5102 | 5202 |
| 6000 | Fedora 42 | 6100 | 6200 |
| 6001 | Fedora 43 | 6101 | 6201 |
| 6002 | Fedora 44 | 6102 | 6202 |
| 7000 | Alma Linux 8 | 7100 | 7200 |
| 7001 | Alma Linux 9 | 7101 | 7201 |
| 7002 | Alma Linux 10 | 7102 | 7202 |
| 8000 | openSUSE Leap 15 | 8100 | 8200 |
| 8001 | openSUSE Leap 16 | 8101 | 8201 |

## Troubleshooting

**CentOS Stream download fails with HTTP 403**

v1.1.2+ resolves dated CentOS cloud images instead of the `-latest` symlink. Update the binary and retry.

**Cannot SSH with password / root login denied**

Cloud images often ship with `PermitRootLogin prohibit-password`. v1.1.2+ forces `PermitRootLogin yes` and `PasswordAuthentication yes` via vendor-data. Re-run the downloader (to refresh the snippet), reinstall the template, and clone a new VM.

**VMID already exists**

- On **this node**: use `--yes`, or answer **Yes** when prompted to replace.
- On **another node**: the tool automatically assigns `base + 100` (then `+200`, …).

**One template failed and nothing else installed**

v1.1.2+ continues with the remaining templates and reports failures at the end.

**Storage not listed**

Verify the storage pool supports `images` or `rootdir` content in Proxmox → Datacenter → Storage.

**Snippets directory missing**

Enable the `snippets` content type on at least one storage pool. The tool writes to `/var/lib/vz/snippets/`.

**Cluster environments**

The tool checks VMIDs across the entire Proxmox cluster. Templates are created on the node where you run the command. If a base VMID is already taken on another node, this node automatically gets `base + 100`, `base + 200`, and so on.

## Custom template list

To use a custom `images.json` file, pass the path or URL as the first argument:

```sh
./downloader_x86 /path/to/images.json
./downloader_x86 https://example.com/images.json
```

If no argument is given, the bundled template list is used.
