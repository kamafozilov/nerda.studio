//! `cargo run -p dev -- install`: builds a release, bundles it as Nerda Studio,
//! signs it and puts it in `/Applications` in place of the one there, then opens
//! it; it takes over the window from whichever instance runs.
//!
//! The signature is ad hoc unless `NERDA_SIGN_IDENTITY` names a code-signing
//! certificate in the keychain. macOS remembers the permissions it granted an
//! ad-hoc signed app for that one build only; signed with the same certificate,
//! every later install keeps them.

use std::{
    ffi::OsStr,
    fs, io,
    path::Path,
    process::{Command, ExitCode},
};

use app_bundle::RELEASE;

const APPLICATIONS: &str = "/Applications";

pub fn run(root: &Path) -> ExitCode {
    match install(root) {
        Ok(target) => {
            println!("install: {} is installed and open", target.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("install: {error}");
            ExitCode::FAILURE
        }
    }
}

fn install(root: &Path) -> io::Result<std::path::PathBuf> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    println!("install: building the release");
    run_tool(
        Command::new(cargo)
            .args(["build", "--release", "-p", "studio"])
            .current_dir(root),
    )?;
    let executable = root.join("target/release/studio");
    let app = app_bundle::beside(&executable, &RELEASE);
    app_bundle::assemble(&executable, &app, &RELEASE, env!("CARGO_PKG_VERSION"))?;
    let identity = std::env::var("NERDA_SIGN_IDENTITY").unwrap_or_else(|_| "-".into());
    run_tool(
        Command::new("codesign")
            .args(["--force", "--sign", &identity])
            .arg(&app),
    )?;

    // Copied in under another name and swapped in whole, so a failure halfway
    // leaves the installed app as it was.
    let target = Path::new(APPLICATIONS).join(format!("{}.app", RELEASE.name));
    let staged = Path::new(APPLICATIONS).join(format!(".{}.app.new", RELEASE.name));
    let previous = Path::new(APPLICATIONS).join(format!(".{}.app.old", RELEASE.name));
    remove_if_present(&staged)?;
    remove_if_present(&previous)?;
    run_tool(Command::new("ditto").arg(&app).arg(&staged))?;
    if target.exists() {
        fs::rename(&target, &previous)?;
    }
    if let Err(error) = fs::rename(&staged, &target) {
        fs::rename(&previous, &target).ok();
        return Err(error);
    }
    remove_if_present(&previous)?;

    // `-n`: a plain `open` would only bring forward an installed copy still running.
    run_tool(Command::new("open").arg("-n").arg(&target))?;
    Ok(target)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Runs a tool to the end; an error names it if it fails.
fn run_tool(command: &mut Command) -> io::Result<()> {
    let name = Path::new(command.get_program())
        .file_name()
        .unwrap_or(OsStr::new("?"))
        .to_string_lossy()
        .into_owned();
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{name} failed ({status})")))
    }
}
