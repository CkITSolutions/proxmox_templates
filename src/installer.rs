use std::path::Path;
use std::process::Stdio;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::process::Command;

use crate::downloader::{build_http_client, download_template, Template};
use crate::util::{destroy_template_vm, template_vm_name};

const DEFAULT_MEMORY_MB: u32 = 2048;
const DEFAULT_DISK_RESIZE: &str = "8G";
const DEFAULT_BRIDGE: &str = "vmbr0";
const DEFAULT_CPU: &str = "host";

pub async fn install_template(
    storage_volume: &str,
    template: &Template,
    image: &Path,
    vendor_snippet: &str,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold.dim} {bar} {percent}% [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Installing {}", template.name));
    pb.set_length(100);
    pb.set_position(10);

    let vm_name = template_vm_name(template);
    let vmid = template.vmid.to_string();

    pb.set_position(20);

    if let Err(err) = install_template_inner(
        storage_volume,
        template,
        image,
        vendor_snippet,
        &vm_name,
        &vmid,
        pb,
    )
    .await
    {
        let _ = destroy_template_vm(template.vmid).await;
        return Err(err);
    }

    pb.set_position(100);
    pb.finish_with_message(format!("Installed {}", template.name));

    Ok(())
}

async fn install_template_inner(
    storage_volume: &str,
    template: &Template,
    image: &Path,
    vendor_snippet: &str,
    vm_name: &str,
    vmid: &str,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    run_qm(
        &[
            "create",
            vmid,
            "--name",
            vm_name,
            "--memory",
            &DEFAULT_MEMORY_MB.to_string(),
            "--cpu",
            DEFAULT_CPU,
            "--net0",
            &format!("virtio,bridge={DEFAULT_BRIDGE}"),
            "--agent",
            "enabled=1",
            "--ostype",
            "l26",
            "--bios",
            "seabios",
            "--machine",
            "q35",
            "--scsihw",
            "virtio-scsi-pci",
            "--serial0",
            "socket",
            "--vga",
            "serial0",
        ],
        template,
    )
    .await?;

    pb.set_position(40);

    run_qm(
        &[
            "disk",
            "import",
            vmid,
            image.to_str().unwrap(),
            storage_volume,
        ],
        template,
    )
    .await?;

    pb.set_position(60);

    let disk_ref = get_imported_disk_ref(vmid, template).await?;
    run_qm(
        &[
            "set",
            vmid,
            "--scsi0",
            &format!("{disk_ref},cache=writeback,discard=on,ssd=1"),
            "--boot",
            "order=scsi0",
            "--ide2",
            &format!("{storage_volume}:cloudinit"),
            "--ipconfig0",
            "ip=dhcp",
            "--citype",
            "nocloud",
            "--cicustom",
            &format!("vendor={vendor_snippet}"),
        ],
        template,
    )
    .await?;

    pb.set_position(80);

    run_qm(
        &["resize", vmid, "scsi0", &format!("+{DEFAULT_DISK_RESIZE}")],
        template,
    )
    .await?;

    run_qm(&["template", vmid], template).await?;

    Ok(())
}

async fn get_imported_disk_ref(vmid: &str, template: &Template) -> anyhow::Result<String> {
    let output = Command::new("qm")
        .args(["config", vmid])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to read VM {vmid} config after disk import: {stderr}"
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let Some((key, value)) = line.split_once(": ") else {
            continue;
        };

        if key.starts_with("unused") {
            return Ok(value.trim().to_string());
        }
    }

    Err(anyhow::anyhow!(
        "No imported disk found for {} after disk import",
        template.name
    ))
}

pub async fn download_and_install_templates(
    tmp_dir: &Path,
    storage_volume: &str,
    templates: Vec<Template>,
    vendor_snippet: &str,
) -> anyhow::Result<()> {
    if templates.is_empty() {
        println!("No templates selected.");
        return Ok(());
    }

    let client = build_http_client()?;
    let mpb = MultiProgress::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut installed = 0usize;

    for template in templates {
        let pb = mpb.add(ProgressBar::new(100));
        pb.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
        pb.set_message(format!("Queued {}", template.name));

        let result = async {
            let file_name = download_template(tmp_dir, &client, &template, &pb).await?;
            install_template(
                storage_volume,
                &template,
                &file_name,
                vendor_snippet,
                &pb,
            )
            .await?;
            let _ = tokio::fs::remove_file(&file_name).await;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        match result {
            Ok(()) => installed += 1,
            Err(err) => {
                pb.finish_with_message(format!("Failed {}", template.name));
                eprintln!("Failed {}: {err}", template.name);
                failures.push((template.name.clone(), err.to_string()));
            }
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    eprintln!(
        "Completed with {} installed, {} failed:",
        installed,
        failures.len()
    );
    for (name, err) in &failures {
        eprintln!("  - {name}: {err}");
    }

    Err(anyhow::anyhow!(
        "{} template(s) failed to install",
        failures.len()
    ))
}

async fn run_qm(args: &[&str], template: &Template) -> anyhow::Result<()> {
    let output = Command::new("qm")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(anyhow::anyhow!(
        "qm {} failed for {}: {}{}",
        args.first().unwrap_or(&""),
        template.name,
        stderr,
        stdout
    ))
}
