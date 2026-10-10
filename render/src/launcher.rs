//! Small desktop entry point. Keep the diagnostic viewer's console subsystem
//! while starting it without a console from a double-click on Windows.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Stdio},
};

fn viewer_command(executable: &Path, rom_path: Option<&OsStr>) -> Command {
    let viewer = if cfg!(target_os = "windows") {
        "rustario64-viewer.exe"
    } else {
        "rustario64-viewer"
    };
    let mut command = Command::new(executable.with_file_name(viewer));
    command
        .arg("launch")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    // A dropped ROM file can prefill selection, but never bypass validation.
    if let Some(path) = rom_path {
        command.arg(path);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
}

fn run() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut args = std::env::args_os().skip(1);
    let rom_path = args.next();
    if args.next().is_some() {
        return Err("Open Rustario64 without arguments, or with one local ROM path.".into());
    }
    let output = viewer_command(&executable, rom_path.as_deref())
        .output()
        .map_err(|e| {
            format!(
                "Could not open Rustario64: {e}. Extract the whole desktop bundle before starting."
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "Rustario64 could not start or stopped unexpectedly ({}).\n{}",
        output.status,
        detail.trim()
    ))
}

fn main() {
    if let Err(error) = run() {
        rfd::MessageDialog::new()
            .set_title("Rustario64 could not start")
            .set_description(&error)
            .set_level(rfd::MessageLevel::Error)
            .show();
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_opens_the_bundled_viewer_and_preserves_rom_paths_as_one_argument() {
        let folder = Path::new("folder with spaces");
        let command = viewer_command(
            &folder.join("rustario64-desktop"),
            Some(OsStr::new("ROM folder/game (US).z64")),
        );
        let viewer = if cfg!(target_os = "windows") {
            "rustario64-viewer.exe"
        } else {
            "rustario64-viewer"
        };
        assert_eq!(command.get_program(), folder.join(viewer));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [OsStr::new("launch"), OsStr::new("ROM folder/game (US).z64")]
        );
        let command = viewer_command(&folder.join("rustario64-desktop"), None);
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [OsStr::new("launch")]
        );
    }
}
