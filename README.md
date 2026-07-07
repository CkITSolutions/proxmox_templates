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
- Builds Proxmox templates with cloud-init, QEMU guest agent, and DHCP networking
- Lets you choose which templates to install
- Works on standalone Proxmox nodes and clusters



## Supported operating systems


| Distribution  | Versions            | VMIDs     |
| ------------- | ------------------- | --------- |
| Ubuntu        | 22.04, 24.04, 26.04 | 1000–1002 |
| CentOS Stream | 9, 10               | 3000–3001 |
| Debian        | 11, 12, 13          | 4000–4002 |
| Rocky Linux   | 8, 9, 10            | 5000–5002 |
| Fedora        | 42, 43, 44          | 6000–6002 |
| Alma Linux    | 8, 9, 10            | 7000–7002 |
| openSUSE Leap | 15, 16              | 8000–8001 |




## Documentation

See [docs/template-downloader.md](docs/template-downloader.md) for full usage instructions, including interactive mode and command-line options.

## Requirements

- Proxmox VE with `qm` and `pvesm` available
- Root access on the Proxmox node
- Storage configured for VM disks and snippets (`/var/lib/vz/snippets`)



## License

Distributed under the AGPL-3.0 License. See `LICENSE` for more information.