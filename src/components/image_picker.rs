use std::{path::PathBuf, thread};
use telorgon::session::{ProcessOutput, Stream};

// Give explicit cancellation distinct statuses from GTK/Zenity startup failures.
const CANCELED: i32 = 10;
const TIMED_OUT: i32 = 11;

pub(crate) fn choose(
    done: impl FnOnce(Result<Option<PathBuf>, String>) + Send + 'static,
) -> Result<(), String> {
    let program = PathBuf::from("/usr/bin/zenity");
    if !program.is_file() {
        return Err("The photo picker is unavailable because Zenity is not installed.".into());
    }
    thread::Builder::new()
        .name("settings-image-picker".into())
        .spawn(move || {
            // The app session supervises and reaps the child. Zenity's timeout bounds the
            // dialog lifetime even if the page closes before the user responds.
            let result = futures_lite::future::block_on(
                telorgon::session::command(program)
                    // The shared portal backend can still belong to another desktop's display.
                    // Keep this helper's chooser on its inherited graphical session instead.
                    .env("GDK_DEBUG", "no-portals")
                    .env("ZENITY_OK", "0")
                    .env("ZENITY_CANCEL", CANCELED.to_string())
                    .env("ZENITY_ESC", CANCELED.to_string())
                    .env("ZENITY_TIMEOUT", TIMED_OUT.to_string())
                    .env("ZENITY_ERROR", "12")
                    .args([
                        "--file-selection",
                        "--title=Add Photo",
                        "--file-filter=Images | *.png *.jpg *.jpeg *.webp *.bmp *.PNG *.JPG *.JPEG *.WEBP *.BMP",
                        "--timeout=120",
                    ])
                    .stdin(Stream::Null)
                    .output_limit(8192)
                    .output(),
            )
            .map_err(|error| browser_error(&error.to_string()))
            .and_then(selection);
            if let Err(error) = &result {
                eprintln!("settings-image-picker: {error}");
            }
            done(result);
        })
        .map(drop)
        .map_err(|_| "Could not start the photo picker. Try Add Photo again.".into())
}

fn selection(output: ProcessOutput) -> Result<Option<PathBuf>, String> {
    if output.status.code() == Some(CANCELED) {
        return Ok(None);
    }
    if output.status.code() == Some(TIMED_OUT) {
        return Err("The photo picker timed out. Try Add Photo again.".into());
    }
    if !output.status.success() || output.stdout_truncated {
        let detail = if output.stdout_truncated {
            "The selected file path was too long to read".into()
        } else if output.stderr.is_empty() {
            format!("Image browser exited with {}", output.status)
        } else {
            String::from_utf8_lossy(&output.stderr).into_owned()
        };
        return Err(browser_error(&detail));
    }
    let mut path = output.stdout;
    // Strip only the dialog's final newline, preserving whitespace in the chosen filename.
    if path.last() == Some(&b'\n') {
        path.pop();
    }
    if path.is_empty() {
        return Ok(None);
    }
    #[cfg(unix)]
    let path = {
        use std::os::unix::ffi::OsStringExt;
        PathBuf::from(std::ffi::OsString::from_vec(path))
    };
    #[cfg(not(unix))]
    let path = PathBuf::from(
        String::from_utf8(path)
            .map_err(|_| "The selected image path could not be read.".to_owned())?,
    );
    Ok(Some(path))
}

fn browser_error(detail: &str) -> String {
    let detail = detail
        .chars()
        .filter(|character| !character.is_control() || character.is_whitespace())
        .take(240)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    format!("Could not open the photo picker: {detail}. Try Add Photo again.")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{os::unix::process::ExitStatusExt, process::ExitStatus};

    fn output(code: i32, stdout: &[u8]) -> ProcessOutput {
        ProcessOutput {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn selected_path_preserves_filename_characters() {
        assert_eq!(
            selection(output(0, b"/tmp/a $(literal) picture \n")).unwrap(),
            Some(PathBuf::from("/tmp/a $(literal) picture "))
        );
        assert_eq!(
            selection(output(0, b"/tmp/picture\n.png\n")).unwrap(),
            Some(PathBuf::from("/tmp/picture\n.png"))
        );
    }

    #[test]
    fn explicit_cancel_does_not_select_a_path() {
        let mut canceled = output(CANCELED, b"/tmp/ignored.png\n");
        canceled.stderr = b"Gtk-WARNING: an unrelated theme warning".to_vec();
        assert_eq!(selection(canceled).unwrap(), None);
        assert_eq!(selection(output(0, b"\n")).unwrap(), None);
    }

    #[test]
    fn startup_failure_is_not_mistaken_for_user_cancellation() {
        let mut failed = output(1, b"");
        failed.stderr = b"Gtk-WARNING: cannot open display: wayland-1\n".to_vec();
        let error = selection(failed).unwrap_err();
        assert!(error.contains("cannot open display"));
        assert!(error.contains("Add Photo"));
        assert!(selection(output(1, b"")).is_err());
    }

    #[test]
    fn timeout_is_visible_and_never_imports_an_image() {
        let error = selection(output(TIMED_OUT, b"/tmp/ignored.png\n")).unwrap_err();
        assert!(error.contains("timed out"));
    }

    #[test]
    fn failure_details_are_bounded_and_do_not_contain_control_characters() {
        let error = browser_error(&format!("\u{1b}[31mFailed\n{}", "x".repeat(2000)));
        assert!(error.len() < 400);
        assert!(!error.chars().any(char::is_control));
    }

    #[test]
    fn failed_or_truncated_picker_does_not_import_partial_paths() {
        assert!(selection(output(2, b"/tmp/image.png\n")).is_err());
        let mut truncated = output(0, b"/tmp/incomplete");
        truncated.stdout_truncated = true;
        assert!(selection(truncated).is_err());
    }
}
