use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::cli::ReleaseAction;
use crate::ui;
use crate::version::{self, BumpKind};

fn version_file() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join("VERSION");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    bail!("VERSION file not found");
}

fn action_to_bump(action: &ReleaseAction) -> (Option<BumpKind>, Option<&str>, bool) {
    match action {
        ReleaseAction::Patch { tag } => (Some(BumpKind::Patch), None, *tag),
        ReleaseAction::Minor { tag } => (Some(BumpKind::Minor), None, *tag),
        ReleaseAction::Major { tag } => (Some(BumpKind::Major), None, *tag),
        ReleaseAction::Set { version, tag } => (None, Some(version.as_str()), *tag),
    }
}

pub fn run(action: &ReleaseAction) -> Result<()> {
    let path = version_file()?;
    let current = version::read_from_file(&path)?;
    let (bump_kind, explicit, create_tag) = action_to_bump(action);

    let new_ver = if let Some(kind) = bump_kind {
        version::bump(&current, kind)
    } else {
        let raw = explicit.context("set requires a version string")?;
        let sem = version::extract_semver(raw)
            .context("invalid semver in provided version")?;
        semver::Version::parse(&sem)?
    };

    version::write_to_file(&path, &new_ver)?;
    ui::ok(&format!("VERSION: {current} -> {new_ver}"));

    if create_tag {
        create_git_tag(&new_ver.to_string())?;
    }

    Ok(())
}

pub fn run_publish(action: &ReleaseAction) -> Result<()> {
    let in_repo = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !in_repo {
        bail!("release-publish must run in a git repo");
    }

    let porcelain = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .context("running git status")?;
    let dirty = String::from_utf8_lossy(&porcelain.stdout);
    if !dirty.trim().is_empty() {
        bail!("working tree is not clean — commit or stash first");
    }

    let action_with_tag = match action {
        ReleaseAction::Patch { .. } => ReleaseAction::Patch { tag: true },
        ReleaseAction::Minor { .. } => ReleaseAction::Minor { tag: true },
        ReleaseAction::Major { .. } => ReleaseAction::Major { tag: true },
        ReleaseAction::Set { version, .. } => ReleaseAction::Set {
            version: version.clone(),
            tag: true,
        },
    };

    run(&action_with_tag)?;

    let path = version_file()?;
    let new_ver = version::read_from_file(&path)?;
    let tag = format!("v{new_ver}");

    let git_add = Command::new("git")
        .args(["add", "VERSION"])
        .status()
        .context("git add VERSION")?;
    if !git_add.success() {
        bail!("git add VERSION failed");
    }

    let msg = format!("release: {tag}");
    let git_commit = Command::new("git")
        .args(["commit", "-m", &msg])
        .status()
        .context("git commit")?;
    if !git_commit.success() {
        bail!("git commit failed");
    }

    let git_push = Command::new("git")
        .args(["push", "origin", "HEAD"])
        .status()
        .context("git push")?;
    if !git_push.success() {
        bail!("git push failed");
    }

    let git_push_tag = Command::new("git")
        .args(["push", "origin", &tag])
        .status()
        .context("git push tag")?;
    if !git_push_tag.success() {
        bail!("git push tag failed");
    }

    ui::ok(&format!("published release {tag}"));
    Ok(())
}

fn create_git_tag(ver: &str) -> Result<()> {
    let tag = format!("v{ver}");

    let exists = Command::new("git")
        .args(["rev-parse", &tag])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if exists {
        bail!("tag already exists: {tag}");
    }

    let status = Command::new("git")
        .args(["tag", &tag])
        .status()
        .context("git tag")?;

    if !status.success() {
        bail!("git tag failed");
    }

    ui::ok(&format!("created tag: {tag}"));
    eprintln!("push tag with: git push origin {tag}");
    Ok(())
}
