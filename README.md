# Ck IT Solutions Template Downloader

A Proxmox tool by **Ck IT Solutions** that downloads official upstream cloud images and turns them into ready-to-use VM templates with cloud-init.

## Quick start

On your Proxmox node:

```sh
wget https://github.com/CkITSolutions/proxmox_templates/releases/latest/download/downloader_x86
chmod +x downloader_x86
./downloader_x86
```

## What it does

- Downloads the latest official cloud images from upstream vendors
- Builds Proxmox templates with cloud-init, QEMU guest agent, DHCP, and host CPU
- Enables SSH key **and** password login (including root)
- Lets you choose which templates to install
- Continues installing remaining templates if one download fails
- Works on standalone Proxmox nodes and multi-node clusters (auto VMID offsets)

## Supported operating systems

| Distribution  | Versions            | Base VMIDs | Node 2 / Node 3 |
| ------------- | ------------------- | ---------- | --------------- |
| Ubuntu        | 22.04, 24.04, 26.04 | 1000–1002  | +100 / +200     |
| CentOS Stream | 9, 10               | 3000–3001  | +100 / +200     |
| Debian        | 11, 12, 13          | 4000–4002  | +100 / +200     |
| Rocky Linux   | 8, 9, 10            | 5000–5002  | +100 / +200     |
| Fedora        | 42, 43, 44          | 6000–6002  | +100 / +200     |
| Alma Linux    | 8, 9, 10            | 7000–7002  | +100 / +200     |
| openSUSE Leap | 15, 16              | 8000–8001  | +100 / +200     |

On a cluster, if the base VMID is already used on another node, this node gets `base + 100`, then `base + 200`, and so on (for example Ubuntu 26.04: `1002` → `1102` → `1202`).

## Documentation

See [docs/template-downloader.md](docs/template-downloader.md) for full usage instructions, including interactive mode, cluster VMIDs, cloud-init/SSH settings, and command-line options.

## Requirements

- Proxmox VE with `qm` and `pvesm` available
- Root access on the Proxmox node
- Storage configured for VM disks and snippets (`/var/lib/vz/snippets`)

## License

Distributed under the AGPL-3.0 License. See `LICENSE` for more information.
