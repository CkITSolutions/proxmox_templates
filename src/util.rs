use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};
use serde_json::Value;

use crate::downloader::{Group, Template};

const SNIPPET_DIR: &str = "/var/lib/vz/snippets";
const VENDOR_SNIPPET: &str = "ckits-vendor-data.yaml";

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

pub fn prepare_templates(templates: Vec<Template>, options: &RunOptions) -> Vec<Template> {
    let mut prepared = Vec::new();

    for template in templates {
        if !is_vmid_used(&template.vmid) {
            prepared.push(template);
            continue;
        }

        if options.assume_yes || confirm_replace(template.vmid, &template.name) {
            if let Err(err) = destroy_vm(template.vmid) {
                eprintln!("Failed to remove existing VM {}: {err}", template.vmid);
                continue;
            }
            prepared.push(template);
        } else {
            println!(
                "Skipping {}: VMID {} already exists",
                template.name, template.vmid
            );
        }
    }

    prepared
}

pub fn confirm_replace(vmid: i32, name: &str) -> bool {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "VMID {vmid} ({name}) already exists. Replace it?"
        ))
        .default(false)
        .interact()
        .unwrap()
}

pub fn ensure_cloud_init_snippet() -> anyhow::Result<String> {
    fs::create_dir_all(SNIPPET_DIR)?;

    let snippet_path = Path::new(SNIPPET_DIR).join(VENDOR_SNIPPET);
    if !snippet_path.exists() {
        let default_snippet = include_str!("../snippets/vendor-data.yaml");
        fs::write(&snippet_path, default_snippet)?;
    }

    Ok(format!("local:snippets/{VENDOR_SNIPPET}"))
}

pub fn destroy_vm(vmid: i32) -> anyhow::Result<()> {
    if is_vmid_used(&vmid) {
        let status = Command::new("qm")
            .args(["destroy", &vmid.to_string()])
            .status()?;

        if !status.success() {
            return Err(anyhow::anyhow!("Failed to destroy VM {vmid}"));
        }
    }

    Ok(())
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
                    resource.get("vmid").and_then(|value| value.as_i64()) == Some(*vmid as i64)
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
