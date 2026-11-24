//! How `torrz` talks to the running app: one JSON [`Command`] a line over a Unix socket in the
//! app's own folder, one JSON [`Response`] a line back.

use std::path::PathBuf;

use crate::command::{Command, Response};

/// `$TORRZ_SOCK`, else `torrz.sock` in the app's folder.
pub fn socket_path() -> PathBuf {
    match std::env::var_os("TORRZ_SOCK") {
        Some(p) => p.into(),
        None => crate::backend::app_dir().join("torrz.sock"),
    }
}

#[cfg(unix)]
mod imp {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::sync::Arc;

    use super::*;

    /// Whether an app is already answering on `path`.
    pub fn is_running(path: &Path) -> bool {
        UnixStream::connect(path).is_ok()
    }

    /// Listens on `path` and answers each command with `handler`, on a thread per connection.
    /// Fails if another app is already answering there.
    pub fn serve(path: &Path, handler: impl Fn(Command) -> Response + Send + Sync + 'static) -> anyhow::Result<()> {
        if is_running(path) {
            anyhow::bail!("Torrz is already running (socket {})", path.display());
        }
        // Nobody answered, so a socket file here is left over from a crash.
        let _ = std::fs::remove_file(path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let listener = UnixListener::bind(path)?;
        // The socket runs commands as the user: keep other users out.
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        let handler = Arc::new(handler);
        std::thread::Builder::new().name("torrz-ipc".into()).spawn(move || {
            for conn in listener.incoming().flatten() {
                let handler = handler.clone();
                let _ = std::thread::Builder::new().name("torrz-ipc-conn".into()).spawn(move || {
                    let Ok(mut out) = conn.try_clone() else { return };
                    for line in BufReader::new(conn).lines() {
                        let Ok(line) = line else { break };
                        if line.trim().is_empty() {
                            continue;
                        }
                        let resp = match serde_json::from_str::<Command>(&line) {
                            Ok(cmd) => handler(cmd),
                            Err(e) => Response::err(format!("bad command: {e}")),
                        };
                        let mut json = serde_json::to_string(&resp).unwrap_or_else(|_| "{\"ok\":false}".into());
                        json.push('\n');
                        if out.write_all(json.as_bytes()).is_err() {
                            break;
                        }
                    }
                });
            }
        })?;
        Ok(())
    }

    pub fn send(path: &Path, cmd: &Command) -> anyhow::Result<Response> {
        let mut s = UnixStream::connect(path).map_err(|e| anyhow::anyhow!("Torrz isn't running ({e}). Start it with `torrz open`."))?;
        let mut json = serde_json::to_string(cmd)?;
        json.push('\n');
        s.write_all(json.as_bytes())?;
        let mut line = String::new();
        BufReader::new(s).read_line(&mut line)?;
        anyhow::ensure!(!line.is_empty(), "Torrz closed the connection without answering");
        Ok(serde_json::from_str(&line)?)
    }
}

#[cfg(not(unix))]
mod imp {
    use std::path::Path;

    use super::*;

    pub fn is_running(_: &Path) -> bool {
        false
    }

    pub fn serve(_: &Path, _: impl Fn(Command) -> Response + Send + Sync + 'static) -> anyhow::Result<()> {
        anyhow::bail!("the command socket is not implemented on this platform yet")
    }

    pub fn send(_: &Path, _: &Command) -> anyhow::Result<Response> {
        anyhow::bail!("the command socket is not implemented on this platform yet")
    }
}

pub use imp::{is_running, send, serve};

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_command_goes_over_the_socket_and_its_answer_comes_back() {
        let dir = std::env::temp_dir().join(format!("torrz-ipc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.sock");
        serve(&path, |c| Response::ok(c.to_cli())).unwrap();
        let r = send(&path, &Command::Pause { targets: vec!["all".into()] }).unwrap();
        assert_eq!((r.ok, r.message.as_str()), (true, "torrz pause all"));
        assert!(serve(&path, |_| Response::ok("")).is_err(), "a second app is refused");
        let _ = std::fs::remove_dir_all(dir);
    }
}
