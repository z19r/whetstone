use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::cli::ReleaseAction;
use crate::ui;
use crate::version::{self, BumpKind};

fn repo_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("finding git root")?;
    if !output.status.success() {
        bail!("not inside a git repository");
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

fn version_file() -> Result<PathBuf> {
    let root = repo_root()?;
    let candidate = root.join("VERSION");
    if candidate.exists() {
        return Ok(candidate);
    }
    bail!("VERSION file not found");
}

fn sync_cargo_toml(root: &std::path::Path, new_ver: &semver::Version) -> Result<()> {
    let cargo_path = root.join("Cargo.toml");
    if !cargo_path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&cargo_path).context("reading Cargo.toml")?;
    let mut in_package = false;
    let mut replaced = false;
    let new_content: String = content
        .lines()
        .map(|line| {
            if line.trim() == "[package]" {
                in_package = true;
            } else if line.starts_with('[') {
                in_package = false;
            }
            if in_package && !replaced && line.trim_start().starts_with("version") {
                if let Some(eq_pos) = line.find('=') {
                    replaced = true;
                    return format!("{}= \"{new_ver}\"", &line[..eq_pos]);
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let new_content = if content.ends_with('\n') && !new_content.ends_with('\n') {
        format!("{new_content}\n")
    } else {
        new_content
    };
    std::fs::write(&cargo_path, new_content).context("writing Cargo.toml")?;
    Ok(())
}

fn action_to_bump(action: &ReleaseAction) -> (Option<BumpKind>, Option<&str>) {
    match action {
        ReleaseAction::Patch => (Some(BumpKind::Patch), None),
        ReleaseAction::Minor => (Some(BumpKind::Minor), None),
        ReleaseAction::Major => (Some(BumpKind::Major), None),
        ReleaseAction::Set { version } => (None, Some(version.as_str())),
    }
}

fn bump_version(action: &ReleaseAction) -> Result<semver::Version> {
    let root = repo_root()?;
    let path = version_file()?;
    let current = version::read_from_file(&path)?;
    let (bump_kind, explicit) = action_to_bump(action);

    let new_ver = if let Some(kind) = bump_kind {
        version::bump(&current, kind)
    } else {
        let raw = explicit.context("set requires a version string")?;
        let sem = version::extract_semver(raw).context("invalid semver in provided version")?;
        semver::Version::parse(&sem)?
    };

    version::write_to_file(&path, &new_ver)?;
    sync_cargo_toml(&root, &new_ver)?;
    ui::ok(&format!("VERSION: {current} -> {new_ver}"));
    Ok(new_ver)
}

fn git_cmd(args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .status()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if !status.success() {
        bail!("git {} failed", args.join(" "));
    }
    Ok(())
}

pub fn run(action: &ReleaseAction) -> Result<()> {
    let new_ver = bump_version(action)?;
    let tag = format!("v{new_ver}");
    let branch = format!("release/{tag}");

    git_cmd(&["checkout", "-b", &branch])?;
    git_cmd(&["add", "VERSION", "Cargo.toml"])?;

    let msg = format!("release: {tag}");
    git_cmd(&["commit", "-m", &msg])?;
    git_cmd(&["push", "-u", "origin", &branch])?;

    let pr_body = format!("Bump version to {new_ver}.\n\nMerging triggers the release workflow.");
    let pr = Command::new("gh")
        .args([
            "pr",
            "create",
            "--title",
            &format!("release: {tag}"),
            "--body",
            &pr_body,
            "--base",
            "main",
            "--head",
            &branch,
        ])
        .output()
        .context("gh pr create")?;
    if !pr.status.success() {
        let stderr = String::from_utf8_lossy(&pr.stderr);
        bail!("gh pr create failed: {stderr}");
    }

    let pr_url = String::from_utf8_lossy(&pr.stdout).trim().to_string();
    ui::ok(&format!("opened release PR: {pr_url}"));
    Ok(())
}

pub fn run_publish(action: &ReleaseAction) -> Result<()> {
    let porcelain = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .context("running git status")?;
    let dirty = String::from_utf8_lossy(&porcelain.stdout);
    if !dirty.trim().is_empty() {
        bail!("working tree is not clean — commit or stash first");
    }

    let new_ver = bump_version(action)?;
    let tag = format!("v{new_ver}");

    git_cmd(&["add", "VERSION", "Cargo.toml"])?;
    git_cmd(&["commit", "-m", &format!("release: {tag}")])?;
    git_cmd(&["tag", &tag])?;
    git_cmd(&["push", "origin", "HEAD"])?;
    git_cmd(&["push", "origin", &tag])?;

    ui::ok(&format!("published release {tag}"));
    Ok(())
}
