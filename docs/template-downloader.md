# Template Downloader

Use the **Ck IT Solutions Template Downloader** to install cloud-init enabled VM templates on your Proxmox node.

## Download the script

Download the latest release binary onto your Proxmox node (via SSH or the Proxmox web terminal):

```sh
wget https://github.com/CkITSolutions/proxmox_templates/releases/latest/download/downloader_x86
```

Make the binary executable:

```sh
chmod +x downloader_x86
```

Run the downloader:

```sh
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
[ ] Fedora 44 (Fedora)
```

You can install one template or many at once.

### Step 2: Choose storage

If more than one storage pool is available, pick the pool where VM disks should be stored. The default is usually `local-lvm`.

Make sure the selected storage is configured to store **Disk image** content in Proxmox.

### Step 3: Replace existing templates (if needed)

If a template VMID already exists on your node or cluster, the tool asks whether you want to replace it. Choose **Yes** to remove the old template and install the new one, or **No** to skip it.

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
| `--templates`, `-t` | Comma-separated list of VMIDs to install |
| `--all` | Install every supported template |
| `--storage`, `-s` | Storage pool for VM disks |
| `--yes`, `-y` | Replace existing VMIDs without prompting |
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
| CentOS Stream | `-latest` image symlink on `cloud.centos.org` |
| Rocky Linux | `.latest` image symlink on `download.rockylinux.org` |
| Alma Linux | `-latest` image symlink on `repo.almalinux.org` |
| openSUSE Leap | Official `-Cloud.qcow2` image symlink |

Re-running the downloader always pulls a fresh image before building the template.

## Cloud-init

Every template is created with:

- A cloud-init drive (`ide2`)
- DHCP networking
- QEMU guest agent pre-installed via a vendor-data snippet
- Serial console for reliable access

The cloud-init snippet is stored at `/var/lib/vz/snippets/ckits-vendor-data.yaml`.

## VMID reference

Use these VMIDs when adding templates to your panel or automation:

| VMID | Template |
| --- | --- |
| 1000 | Ubuntu 22.04 |
| 1001 | Ubuntu 24.04 |
| 1002 | Ubuntu 26.04 |
| 3000 | CentOS Stream 9 |
| 3001 | CentOS Stream 10 |
| 4000 | Debian 11 |
| 4001 | Debian 12 |
| 4002 | Debian 13 |
| 5000 | Rocky Linux 8 |
| 5001 | Rocky Linux 9 |
| 5002 | Rocky Linux 10 |
| 6000 | Fedora 42 |
| 6001 | Fedora 43 |
| 6002 | Fedora 44 |
| 7000 | Alma Linux 8 |
| 7001 | Alma Linux 9 |
| 7002 | Alma Linux 10 |
| 8000 | openSUSE Leap 15 |
| 8001 | openSUSE Leap 16 |

## Troubleshooting

**VMID already exists**

Use `--yes` in non-interactive mode, or answer **Yes** when prompted in interactive mode.

**Storage not listed**

Verify the storage pool supports `images` or `rootdir` content in Proxmox → Datacenter → Storage.

**Snippets directory missing**

Enable the `snippets` content type on at least one storage pool. The tool writes to `/var/lib/vz/snippets/`.

**Cluster environments**

The tool checks VMIDs across the entire Proxmox cluster. Templates are created on the node where you run the command.

## Custom template list

To use a custom `images.json` file, pass the path or URL as the first argument:

```sh
./downloader_x86 /path/to/images.json
./downloader_x86 https://example.com/images.json
```

If no argument is given, the bundled template list is used.
