use futures_lite::StreamExt;
use std::{collections::HashMap, path::PathBuf, thread, time::Duration};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

type Options = HashMap<String, OwnedValue>;
const DESKTOP: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";

pub(crate) fn choose(
    done: impl FnOnce(Result<Option<PathBuf>, String>) + Send + 'static,
) -> Result<(), String> {
    thread::Builder::new()
        .name("settings-image-picker".into())
        .spawn(move || {
            let result = futures_lite::future::block_on(open()).map_err(|detail| browser_error(&detail));
            if let Err(error) = &result { eprintln!("settings-image-picker: {error}"); }
            done(result);
        })
        .map(drop)
        .map_err(|_| "Could not start the photo picker. Try Add Photo again.".into())
}

async fn open() -> Result<Option<PathBuf>, String> {
    let connection = zbus::connection::Builder::session().map_err(detail)?
        .method_timeout(Duration::from_secs(10)).build().await.map_err(detail)?;
    let desktop = zbus::Proxy::new(&connection, DESKTOP, PATH, "org.freedesktop.portal.FileChooser")
        .await.map_err(detail)?;
    // Subscribe before OpenFile: a fast response can precede its method reply.
    // This connection owns one request, so the token only needs to be unique here.
    let sender = connection.unique_name().ok_or("The user bus has no connection identity")?
        .as_str().trim_start_matches(':').replace('.', "_");
    let path = format!("{PATH}/request/{sender}/settings_photo");
    let request = zbus::Proxy::new(&connection, DESKTOP, path.as_str(), "org.freedesktop.portal.Request")
        .await.map_err(detail)?;
    let mut responses = request.receive_signal("Response").await.map_err(detail)?;
    let mut options = Options::new();
    options.insert("handle_token".into(), OwnedValue::from(zbus::zvariant::Str::from("settings_photo")));
    options.insert("multiple".into(), OwnedValue::from(false));
    let rules: Vec<(u32, String)> = ["png", "jpg", "jpeg", "webp", "bmp"].into_iter()
        .flat_map(|extension| [format!("*.{extension}"), format!("*.{}", extension.to_uppercase())])
        .map(|pattern| (0, pattern)).collect();
    let filters = vec![("Images".to_owned(), rules)];
    options.insert("filters".into(), OwnedValue::try_from(Value::new(filters)).map_err(detail)?);
    let handle: OwnedObjectPath = desktop.call("OpenFile", &("", "Add Photo", options)).await.map_err(detail)?;
    if handle.as_str() != path {
        let actual = zbus::Proxy::new(&connection, DESKTOP, handle.as_str(), "org.freedesktop.portal.Request")
            .await.map_err(detail)?;
        let _ = actual.call::<_, _, ()>("Close", &()).await;
        return Err("The photo picker returned an unexpected request identity".into());
    }
    let response = futures_lite::future::race(
        async { responses.next().await },
        async { async_io::Timer::after(Duration::from_secs(120)).await; None },
    ).await;
    let Some(response) = response else {
        let _ = request.call::<_, _, ()>("Close", &()).await;
        return Err("The photo picker timed out or disconnected. Try Add Photo again".into());
    };
    let (status, results): (u32, Options) = response.body().deserialize().map_err(detail)?;
    selection(status, results)
}

fn selection(status: u32, mut results: Options) -> Result<Option<PathBuf>, String> {
    match status {
        1 => return Ok(None),
        0 => {},
        _ => return Err("The photo picker could not select a file".into()),
    }
    let uris = Vec::<String>::try_from(results.remove("uris").ok_or("The photo picker returned no file")?)
        .map_err(detail)?;
    let [uri] = uris.as_slice() else { return Err("Select exactly one image".into()) };
    local_path(uri).map(Some)
}

fn local_path(uri: &str) -> Result<PathBuf, String> {
    let path = uri.strip_prefix("file://").ok_or("The photo picker must select a local file")?;
    let path = path.strip_prefix("localhost").unwrap_or(path);
    if !path.starts_with('/') || path.starts_with("//") || path.contains(['?', '#']) || path.len() > 8192 {
        return Err("The selected file URI is invalid".into());
    }
    let mut bytes = Vec::with_capacity(path.len());
    let mut input = path.as_bytes().iter().copied();
    while let Some(byte) = input.next() {
        bytes.push(if byte == b'%' {
            let high = input.next().and_then(hex).ok_or("The selected file URI has an invalid escape")?;
            let low = input.next().and_then(hex).ok_or("The selected file URI has an invalid escape")?;
            high * 16 + low
        } else { byte });
    }
    if bytes.contains(&0) { return Err("The selected file path contains a NUL byte".into()); }
    #[cfg(unix)]
    { use std::os::unix::ffi::OsStringExt; Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes))) }
    #[cfg(not(unix))]
    { Ok(PathBuf::from(String::from_utf8(bytes).map_err(detail)?)) }
}
fn hex(byte: u8) -> Option<u8> { (byte as char).to_digit(16).map(|value| value as u8) }
fn detail(error: impl std::fmt::Display) -> String { error.to_string() }
fn browser_error(detail: &str) -> String {
    let detail = detail.chars().filter(|character| !character.is_control() || character.is_whitespace())
        .take(240).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    format!("Could not open the photo picker: {detail}. Try Add Photo again.")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn local_uri_preserves_filename_bytes() {
        assert_eq!(local_path("file:///tmp/a%20%24%28literal%29%20picture%20%0A.png").unwrap(),
            PathBuf::from("/tmp/a $(literal) picture \n.png"));
        assert_eq!(local_path("file://localhost/tmp/%FF.png").unwrap().as_os_str().as_bytes(), b"/tmp/\xff.png");
    }
    #[test]
    fn remote_malformed_and_nul_uris_are_rejected() {
        for uri in ["https://example.com/photo.png", "file://host/tmp/a", "file://localhostevil/a",
            "file:////host/a", "file:///tmp/%", "file:///tmp/%GG", "file:///tmp/a%00.png", "file:///tmp/a?query", "file:///tmp/a#fragment"] {
            assert!(local_path(uri).is_err(), "{uri}");
        }
    }
    #[test]
    fn cancellation_and_failure_never_select_an_image() {
        assert_eq!(selection(1, Options::new()).unwrap(), None);
        assert!(selection(2, Options::new()).is_err());
        assert!(selection(0, Options::new()).is_err());
        let mut options = Options::new();
        options.insert("uris".into(), OwnedValue::try_from(Value::new(vec!["file:///a", "file:///b"])).unwrap());
        assert!(selection(0, options).is_err());
    }
    #[test]
    fn failure_details_are_bounded_and_have_no_control_characters() {
        let error = browser_error(&format!("\u{1b}[31mFailed\n{}", "x".repeat(2000)));
        assert!(error.len() < 400);
        assert!(!error.chars().any(char::is_control));
    }
}
