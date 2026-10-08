//! Session-managed clipboard helpers. User text is stdin, never a shell command or argument.
use std::{path::PathBuf, thread};
fn helper(read: bool) -> Result<telorgon::session::Command, String> {
    let (program, args): (&str, &[&str]) = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        if read {
            ("wl-paste", &["--no-newline"])
        } else {
            ("wl-copy", &[])
        }
    } else if read {
        ("xclip", &["-selection", "clipboard", "-o"])
    } else {
        ("xclip", &["-selection", "clipboard"])
    };
    let executable = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|path| path.join(program))
        .find(|path| path.is_file())
        .ok_or_else(|| format!("Clipboard helper {program} is unavailable"))?;
    Ok(telorgon::session::command(PathBuf::from(executable)).args(args.iter().copied()))
}
pub(crate) fn copy(text: String, done: impl FnOnce(Result<(), String>) + Send + 'static) {
    thread::spawn(move || {
        let result = helper(false)
            .and_then(|command| {
                futures_lite::future::block_on(
                    command
                        .input(text.into_bytes())
                        .stdout(telorgon::session::Stream::Null)
                        .stderr(telorgon::session::Stream::Null)
                        .status(),
                )
                .map_err(|_| "Could not access clipboard".into())
            })
            .and_then(|output| {
                if output.success() {
                    Ok(())
                } else {
                    Err("Could not copy to clipboard".into())
                }
            });
        done(result);
    });
}
pub(crate) fn read(done: impl FnOnce(Result<String, String>) + Send + 'static) {
    thread::spawn(move || {
        let result = helper(true)
            .and_then(|command| {
                futures_lite::future::block_on(command.output())
                    .map_err(|_| "Could not access clipboard".into())
            })
            .and_then(|output| {
                if output.status.success() && !output.stdout_truncated {
                    String::from_utf8(output.stdout)
                        .map_err(|_| "Clipboard does not contain text".into())
                } else {
                    Err("Could not read clipboard".into())
                }
            });
        done(result);
    });
}
