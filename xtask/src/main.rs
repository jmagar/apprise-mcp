use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(String::as_str).unwrap_or("help");

    match task {
        "dist" => dist(),
        "ci" => ci(),
        "symlink-docs" => symlink_docs(),
        "check-env" => check_env(),
        _ => {
            eprintln!("Usage: cargo xtask <dist|ci|symlink-docs|check-env>");
            std::process::exit(1);
        }
    }
}

/// Build release binary and copy to bin/ for Git LFS tracking.
fn dist() -> Result<()> {
    let root = project_root();
    println!("xtask dist: building release binary...");

    let status = Command::new("cargo")
        .args(["build", "--release", "--locked"])
        .current_dir(&root)
        .status()?;
    check_status("cargo build --release", status)?;

    let src = root.join("target/release/apprise");
    let bin_dir = root.join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    let dst = bin_dir.join("apprise");
    std::fs::copy(&src, &dst)?;
    println!("xtask dist: copied {} -> {}", src.display(), dst.display());
    println!("xtask dist: done. Run `git lfs track` and commit bin/apprise.");
    Ok(())
}

/// Run all CI checks: fmt, clippy, nextest, taplo, audit.
fn ci() -> Result<()> {
    let root = project_root();
    println!("xtask ci: running all checks...");

    // fmt
    check_status(
        "cargo fmt --check",
        Command::new("cargo")
            .args(["fmt", "--", "--check"])
            .current_dir(&root)
            .status()?,
    )?;

    // clippy
    check_status(
        "cargo clippy",
        Command::new("cargo")
            .args(["clippy", "--", "-D", "warnings"])
            .current_dir(&root)
            .status()?,
    )?;

    // nextest (fall back to cargo test if nextest not installed)
    let nextest = Command::new("cargo")
        .args(["nextest", "run", "--profile", "ci"])
        .current_dir(&root)
        .status();
    match nextest {
        Ok(s) => check_status("cargo nextest run", s)?,
        Err(_) => {
            eprintln!("cargo-nextest not found, falling back to cargo test");
            check_status(
                "cargo test",
                Command::new("cargo")
                    .args(["test"])
                    .current_dir(&root)
                    .status()?,
            )?;
        }
    }

    // taplo check (optional — skip if not installed)
    let taplo = Command::new("taplo")
        .args(["check"])
        .current_dir(&root)
        .status();
    match taplo {
        Ok(s) => check_status("taplo check", s)?,
        Err(_) => eprintln!("taplo not found — skipping TOML format check"),
    }

    println!("xtask ci: all checks passed.");
    Ok(())
}

/// Symlink AGENTS.md and GEMINI.md to every CLAUDE.md in the repo.
fn symlink_docs() -> Result<()> {
    let root = project_root();
    println!("xtask symlink-docs: symlinking AGENTS.md + GEMINI.md -> CLAUDE.md ...");

    // Walk for CLAUDE.md files (skip .git and target)
    let claude_files = find_claude_mds(&root)?;
    for claude_md in &claude_files {
        let dir = claude_md.parent().unwrap();
        for link_name in &["AGENTS.md", "GEMINI.md"] {
            let link = dir.join(link_name);
            if link.exists() || link.symlink_metadata().is_ok() {
                std::fs::remove_file(&link).ok();
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink("CLAUDE.md", &link)?;
            println!("  {} -> CLAUDE.md", link.display());
        }
    }
    println!(
        "xtask symlink-docs: done ({} CLAUDE.md files processed).",
        claude_files.len()
    );
    Ok(())
}

/// Validate that required environment variables are set.
fn check_env() -> Result<()> {
    println!("xtask check-env: validating required environment variables...");
    let mut missing = Vec::new();
    let required = ["APPRISE_URL"];
    for var in &required {
        match std::env::var(var) {
            Ok(v) if !v.is_empty() => println!("  OK  {var}"),
            _ => {
                eprintln!("  MISSING  {var}");
                missing.push(*var);
            }
        }
    }
    if !missing.is_empty() {
        eprintln!(
            "\nxtask check-env: {} required variable(s) not set.",
            missing.len()
        );
        std::process::exit(1);
    }
    println!("xtask check-env: all required variables are set.");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn project_root() -> PathBuf {
    // Walk up from CARGO_MANIFEST_DIR to find the workspace root
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("xtask has parent").to_path_buf()
}

fn check_status(cmd: &str, status: ExitStatus) -> Result<()> {
    if !status.success() {
        return Err(format!("{cmd} exited with status {status}").into());
    }
    Ok(())
}

fn find_claude_mds(root: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    find_claude_mds_inner(root, &mut result)?;
    Ok(result)
}

fn find_claude_mds_inner(dir: &Path, acc: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip hidden dirs that are not our project dirs
        if name_str.starts_with('.') && name_str != ".claude" {
            continue;
        }
        if name_str == "target" || name_str == "node_modules" {
            continue;
        }
        if path.is_dir() {
            find_claude_mds_inner(&path, acc)?;
        } else if name_str == "CLAUDE.md" {
            acc.push(path);
        }
    }
    Ok(())
}
