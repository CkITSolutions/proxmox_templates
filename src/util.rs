use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};
use serde_json::Value;

use crate::downloader::{Group, Template};

/// Per-node VMID stride in a cluster (1002 → 1102 → 1202).
const VMID_NODE_STRIDE: i32 = 100;

const SNIPPET_DIR: &str = "/var/lib/vz/snippets";
const VENDOR_SNIPPET: &str = "ckits-vendor-data.yaml";

pub fn collect_template_vmids(groups: &[Group]) -> Vec<i32> {
    groups
        .iter()
        .flat_map(|group| group.templates.iter().map(|template| template.vmid))
        .collect()
}

/// Base catalog VMIDs and per-node offsets (base + n*100) within the same thousand band.
pub fn is_managed_template_vmid(vmid: i32, allowed_vmids: &[i32]) -> bool {
    allowed_vmids
        .iter()
        .any(|base| is_vmid_in_base_series(vmid, *base))
}

fn is_vmid_in_base_series(vmid: i32, base: i32) -> bool {
    if vmid < base {
        return false;
    }
    let delta = vmid - base;
    if delta % VMID_NODE_STRIDE != 0 {
        return false;
    }
    vmid < vmid_series_limit(base)
}

fn vmid_series_limit(base: i32) -> i32 {
    (base / 1000 + 1) * 1000
}

fn next_cluster_vmid(
    base: i32,
    allowed_vmids: &[i32],
    reserved: &HashSet<i32>,
) -> Option<i32> {
    let limit = vmid_series_limit(base);
    let mut candidate = base;

    while candidate < limit {
        let conflicts_other_base = allowed_vmids
            .iter()
            .any(|other| *other != base && *other == candidate);

        if !conflicts_other_base && !reserved.contains(&candidate) && !is_vmid_used(&candidate)
        {
            return Some(candidate);
        }

        candidate += VMID_NODE_STRIDE;
    }

    None
}

#[derive(Debug, Default)]
pub struct RunOptions {
    pub storage: Option<String>,
    pub template_vmids: Vec<i32>,
    pub install_all: bool,
    pub assume_yes: bool,
    pub non_interactive: bool,
}

pub fn show_branding() {
    println!();
    println!("Proxmox Template Downloader");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Maintained by Ck IT Solutions");
    println!();
}

pub fn parse_args() -> RunOptions {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut options = RunOptions::default();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--storage" | "-s" => {
                index += 1;
                options.storage = args.get(index).cloned();
            }
            "--templates" | "-t" => {
                index += 1;
                if let Some(value) = args.get(index) {
                    options.template_vmids = value
                        .split(',')
                        .filter_map(|part| part.trim().parse().ok())
                        .collect();
                }
            }
            "--all" => options.install_all = true,
            "--yes" | "-y" => options.assume_yes = true,
            "--non-interactive" => options.non_interactive = true,
            _ => {}
        }
        index += 1;
    }

    options
}

pub fn get_storage_location(options: &RunOptions) -> String {
    if let Some(storage) = &options.storage {
        return storage.clone();
    }

    if options.non_interactive {
        return "local-lvm".to_string();
    }

    let storages = list_vm_storages();
    if storages.is_empty() {
        return Input::new()
            .with_prompt("Storage volume for VM disks")
            .default("local-lvm".into())
            .interact_text()
            .unwrap();
    }

    let default_idx = storages
        .iter()
        .position(|s| s == "local-lvm")
        .unwrap_or(0);

    Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select storage volume for VM disks")
        .default(default_idx)
        .items(&storages)
        .interact()
        .map(|idx| storages[idx].clone())
        .unwrap()
}

pub fn select_templates(groups: &[Group], options: &RunOptions) -> Vec<Template> {
    let mut all_templates = Vec::new();
    for group in groups {
        all_templates.extend(group.templates.iter().cloned());
    }

    if options.install_all {
        return all_templates;
    }

    if !options.template_vmids.is_empty() {
        return all_templates
            .into_iter()
            .filter(|template| options.template_vmids.contains(&template.vmid))
            .collect();
    }

    if options.non_interactive {
        return Vec::new();
    }

    let mut labels = Vec::new();
    let mut templates = Vec::new();

    for group in groups {
        for template in &group.templates {
            labels.push(format!("{} ({})", template.name, group.name));
            templates.push(template.clone());
        }
    }

    if labels.is_empty() {
        return Vec::new();
    }

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select templates to install (space to toggle, enter to confirm)")
        .items(&labels)
        .interact()
        .unwrap();

    selections
        .into_iter()
        .map(|idx| templates[idx].clone())
        .collect()
}

pub fn prepare_templates(
    templates: Vec<Template>,
    options: &RunOptions,
    allowed_vmids: &[i32],
) -> Vec<Template> {
    let mut prepared = Vec::new();
    let mut reserved = HashSet::new();

    for mut template in templates {
        let base_vmid = template.vmid;

        if !allowed_vmids.contains(&base_vmid) {
            eprintln!(
                "Skipping {}: VMID {} is not a managed template ID",
                template.name, base_vmid
            );
            continue;
        }

        // Prefer replacing a local copy already in this template's VMID series.
        if let Some(local_vmid) = find_local_series_vmid(base_vmid, &reserved) {
            let should_replace = if options.assume_yes {
                true
            } else if options.non_interactive {
                false
            } else {
                confirm_replace(local_vmid, &template.name)
            };

            if should_replace {
                if let Err(err) = destroy_template_vm_sync(local_vmid, allowed_vmids) {
                    eprintln!(
                        "Failed to remove existing template {}: {err}",
                        local_vmid
                    );
                    continue;
                }
                template.vmid = local_vmid;
                reserved.insert(local_vmid);
                prepared.push(template);
                continue;
            }
        }

        match next_cluster_vmid(base_vmid, allowed_vmids, &reserved) {
            Some(vmid) => {
                if vmid != base_vmid {
                    println!(
                        "{}: VMID {base_vmid} is already used in the cluster; installing as VMID {vmid}",
                        template.name
                    );
                }
                template.vmid = vmid;
                reserved.insert(vmid);
                prepared.push(template);
            }
            None => {
                eprintln!(
                    "Skipping {}: no free cluster VMID left in series {} (+{} … < {})",
                    template.name,
                    base_vmid,
                    VMID_NODE_STRIDE,
                    vmid_series_limit(base_vmid)
                );
            }
        }
    }

    prepared
}

fn find_local_series_vmid(base: i32, reserved: &HashSet<i32>) -> Option<i32> {
    let limit = vmid_series_limit(base);
    let mut candidate = base;

    while candidate < limit {
        if !reserved.contains(&candidate) && is_vmid_used(&candidate) && is_vmid_local(&candidate)
        {
            return Some(candidate);
        }
        candidate += VMID_NODE_STRIDE;
    }

    None
}

pub fn confirm_replace(vmid: i32, name: &str) -> bool {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "VMID {vmid} ({name}) already exists on this node. Replace it?"
        ))
        .default(false)
        .interact()
        .unwrap()
}

pub fn ensure_cloud_init_snippet() -> anyhow::Result<String> {
    fs::create_dir_all(SNIPPET_DIR)?;

    let snippet_path = Path::new(SNIPPET_DIR).join(VENDOR_SNIPPET);
    let default_snippet = include_str!("../snippets/vendor-data.yaml");
    let needs_update = match fs::read_to_string(&snippet_path) {
        Ok(existing) => existing != default_snippet,
        Err(_) => true,
    };

    if needs_update {
        fs::write(&snippet_path, default_snippet)?;
    }

    Ok(format!("local:snippets/{VENDOR_SNIPPET}"))
}

pub fn destroy_template_vm_sync(vmid: i32, allowed_vmids: &[i32]) -> anyhow::Result<()> {
    if !is_managed_template_vmid(vmid, allowed_vmids) {
        return Err(anyhow::anyhow!(
            "Refusing to destroy VM {vmid}: not a managed template VMID"
        ));
    }

    if is_vmid_used(&vmid) {
        let status = Command::new("qm")
            .args(["destroy", &vmid.to_string()])
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to destroy template VM {vmid}"));
        }
    }

    Ok(())
}

pub async fn destroy_template_vm(vmid: i32) -> anyhow::Result<()> {
    let output = Command::new("qm")
        .args(["config", &vmid.to_string()])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let status = Command::new("qm")
                .args(["destroy", &vmid.to_string()])
                .status()?;

            if !status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to clean up partial template VM {vmid}"
                ));
            }
        }
    }

    Ok(())
}

pub fn is_template_vmid_in_use(vmid: &i32, allowed_vmids: &[i32]) -> bool {
    if !is_managed_template_vmid(*vmid, allowed_vmids) {
        return false;
    }

    is_vmid_used(vmid)
}

fn current_node_name() -> Option<String> {
    let output = Command::new("hostname").arg("-s").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn cluster_vm_owner(vmid: i32) -> Option<String> {
    let output = Command::new("pvesh")
        .args([
            "get",
            "/cluster/resources",
            "--type",
            "vm",
            "--output-format",
            "json",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let resources = serde_json::from_str::<Vec<Value>>(&stdout).ok()?;
    resources.iter().find_map(|resource| {
        let id = resource.get("vmid").and_then(|value| value.as_i64())?;
        if id != i64::from(vmid) {
            return None;
        }
        resource
            .get("node")
            .and_then(|value| value.as_str())
            .map(str::to_string)
    })
}

pub fn is_vmid_local(vmid: &i32) -> bool {
    let Some(current) = current_node_name() else {
        // Fall back to local config presence when hostname lookup fails.
        return PathBuf::from("/etc/pve/qemu-server/")
            .join(format!("{vmid}.conf"))
            .exists();
    };

    match cluster_vm_owner(*vmid) {
        Some(owner) => owner.eq_ignore_ascii_case(&current),
        None => PathBuf::from("/etc/pve/qemu-server/")
            .join(format!("{vmid}.conf"))
            .exists(),
    }
}

pub fn is_vmid_used(vmid: &i32) -> bool {
    let output = Command::new("pvesh")
        .args([
            "get",
            "/cluster/resources",
            "--type",
            "vm",
            "--output-format",
            "json",
        ])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(resources) = serde_json::from_str::<Vec<Value>>(&stdout) {
                return resources.iter().any(|resource| {
                    resource.get("vmid").and_then(|value| value.as_i64()) == Some(i64::from(*vmid))
                });
            }
        }
    }

    let file_path = PathBuf::from("/etc/pve/qemu-server/").join(format!("{vmid}.conf"));
    file_path.exists()
}

pub fn template_vm_name(template: &Template) -> String {
    template
        .name
        .to_lowercase()
        .replace(' ', "-")
        .replace('.', "")
}

fn list_vm_storages() -> Vec<String> {
    let output = Command::new("pvesm")
        .args(["status", "--content", "images,rootdir"])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "Name")
        .map(str::to_string)
        .collect()
}
