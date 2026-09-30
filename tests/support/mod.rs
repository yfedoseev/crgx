//! Test harness for the end-to-end tests: a mock upstream HTTP server, a
//! SOCKS5 + HTTP proxy, fixture builders and a stub `cargo`.

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// GitHub repository URL used by fixtures (only owner/repo are parsed from it).
pub const GH_REPO: &str = "https://github.com/acme/tool";

/// Hostname that does not resolve locally; only the test proxy knows it.
pub const PROXY_ONLY_HOST: &str = "crgx-mock.invalid";

const EXE: &str = if cfg!(windows) { ".exe" } else { "" };

pub fn host_target() -> &'static str {
    env!("CRGX_TARGET")
}

/// The first fallback crgx should try after the host target, if any.
pub fn fallback_target() -> Option<String> {
    let host = host_target();
    if host.contains("-linux-gnu") {
        Some(host.replacen("-linux-gnu", "-linux-musl", 1))
    } else if host.contains("-linux-musl") && has_glibc() {
        Some(host.replacen("-linux-musl", "-linux-gnu", 1))
    } else if host.ends_with("-apple-darwin") {
        Some("universal-apple-darwin".to_string())
    } else if host.ends_with("-pc-windows-msvc") {
        Some(host.replace("-msvc", "-gnu"))
    } else {
        None
    }
}

/// Whether a real (non-gcompat) glibc loader for the host arch exists.
/// Written independently of crgx's own check on purpose.
pub fn has_glibc() -> bool {
    let loader = match host_target().split('-').next() {
        Some("x86_64") => "ld-linux-x86-64.so.2",
        Some("aarch64") => "ld-linux-aarch64.so.1",
        _ => return false,
    };
    // Real glibc defines GLIBC_* versions; Alpine's gcompat shim doesn't
    // and references musl's loader instead.
    ["/lib64", "/lib", "/usr/lib64", "/usr/lib"]
        .iter()
        .any(|d| {
            std::fs::read(Path::new(d).join(loader)).is_ok_and(|b| {
                b.windows(6).any(|w| w == b"GLIBC_") && !b.windows(7).any(|w| w == b"ld-musl")
            })
        })
}

/// A `crgx` command wired to `mock`, with an isolated cache and no ambient proxy.
pub fn crgx(mock: &MockServer, cache: &Path) -> assert_cmd::Command {
    let base = mock.url();
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("crgx");
    cmd.env("CRGX_CACHE_DIR", cache)
        .env("CRGX_CRATES_IO_API", format!("{base}/api/v1"))
        .env("CRGX_CRATES_STATIC", format!("{base}/crates"))
        .env("CRGX_GITHUB_API", format!("{base}/gh"))
        .env("CRGX_QUICKINSTALL_URL", format!("{base}/qi"))
        .timeout(Duration::from_secs(120));
    for var in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY"] {
        cmd.env_remove(var).env_remove(var.to_lowercase());
    }
    cmd
}

/// A local port with nothing listening on it.
pub fn closed_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

pub fn cache_metadata(cache: &Path, key: &str, version: &str) -> serde_json::Value {
    let path = cache
        .join("bin")
        .join(key)
        .join(version)
        .join("metadata.json");
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&data).unwrap()
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn tar_gz(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::none());
    let mut tar = tar::Builder::new(gz);
    for (path, data, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        tar.append_data(&mut header, path, *data).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

/// A release archive whose `bin` entry is the crgx binary itself.
pub fn tool_archive(bin: &str) -> Vec<u8> {
    tool_archive_at(bin)
}

/// Like [`tool_archive`], with the binary at `path` (plus `.exe` on Windows).
pub fn tool_archive_at(path: &str) -> Vec<u8> {
    let exe = std::fs::read(assert_cmd::cargo::cargo_bin!("crgx")).unwrap();
    tar_gz(&[(&format!("{path}{EXE}"), &exe, 0o755)])
}

/// Like [`tool_archive`], compressed as `.tar.xz`.
pub fn tool_archive_xz(bin: &str) -> Vec<u8> {
    use std::io::Write;
    let exe = std::fs::read(assert_cmd::cargo::cargo_bin!("crgx")).unwrap();
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(exe.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    tar.append_data(&mut header, format!("{bin}{EXE}"), &exe[..])
        .unwrap();
    let mut xz = liblzma::write::XzEncoder::new(Vec::new(), 0);
    xz.write_all(&tar.into_inner().unwrap()).unwrap();
    xz.finish().unwrap()
}

/// Serve crates.io metadata and the `.crate` file for a single-binary crate.
pub fn publish_crate(
    mock: &MockServer,
    name: &str,
    version: &str,
    repository: Option<&str>,
    cargo_toml_extra: &str,
) {
    let api = serde_json::json!({
        "crate": { "id": name, "repository": repository },
        "versions": [{ "num": version, "yanked": false, "bin_names": [name] }],
    });
    mock.route(
        &format!("/api/v1/crates/{name}"),
        api.to_string().into_bytes(),
    );

    let cargo_toml =
        format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\n\n{cargo_toml_extra}");
    let krate = tar_gz(&[(
        &format!("{name}-{version}/Cargo.toml"),
        cargo_toml.as_bytes(),
        0o644,
    )]);
    mock.route(&format!("/crates/{name}/{name}-{version}.crate"), krate);
}

/// Serve a GitHub release for `acme/<name>` with the given assets.
pub fn publish_release(mock: &MockServer, name: &str, tag: &str, assets: &[(&str, Vec<u8>)]) {
    let base = mock.url();
    let json = serde_json::json!({
        "tag_name": tag,
        "assets": assets.iter().map(|(n, _)| serde_json::json!({
            "name": n,
            "browser_download_url": format!("{base}/dl/{n}"),
        })).collect::<Vec<_>>(),
    });
    mock.route(
        &format!("/gh/repos/acme/{name}/releases/tags/{tag}"),
        json.to_string().into_bytes(),
    );
    for (n, data) in assets {
        mock.route(&format!("/dl/{n}"), data.clone());
    }
}

// ---------------------------------------------------------------------------
// Mock upstream server
// ---------------------------------------------------------------------------

/// Minimal HTTP/1.1 server: fixed routes, 404 otherwise, request log.
pub struct MockServer {
    addr: SocketAddr,
    host: String,
    routes: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    log: Arc<Mutex<Vec<String>>>,
}

impl MockServer {
    pub fn start() -> Self {
        Self::start_with_host("127.0.0.1")
    }

    /// Start a server whose URLs use `host` (e.g. a name only a proxy resolves).
    pub fn start_with_host(host: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let routes: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let (r, l) = (routes.clone(), log.clone());
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (r, l) = (r.clone(), l.clone());
                thread::spawn(move || {
                    let _ = serve(stream, &r, &l);
                });
            }
        });
        MockServer {
            addr,
            host: host.to_string(),
            routes,
            log,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.addr.port())
    }

    pub fn route(&self, path: &str, body: Vec<u8>) {
        self.routes.lock().unwrap().insert(path.to_string(), body);
    }

    /// Requests received so far, as `"METHOD /path"`.
    pub fn requests(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    pub fn requests_matching(&self, needle: &str) -> Vec<String> {
        self.requests()
            .into_iter()
            .filter(|r| r.contains(needle))
            .collect()
    }
}

fn serve(
    stream: TcpStream,
    routes: &Mutex<HashMap<String, Vec<u8>>>,
    log: &Mutex<Vec<String>>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut stream = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let target = parts.next().unwrap_or("").to_string();
        // Skip headers
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
                break;
            }
        }
        // Absolute-form (from an HTTP proxy): strip scheme and authority.
        let path = match target.split_once("://") {
            Some((_, rest)) => rest.find('/').map_or("/", |i| &rest[i..]).to_string(),
            None => target,
        };
        log.lock().unwrap().push(format!("{method} {path}"));

        let body = routes.lock().unwrap().get(&path).cloned();
        let (status, body) = match body {
            Some(b) => ("200 OK", b),
            None => ("404 Not Found", Vec::new()),
        };
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n",
            body.len()
        )?;
        if method != "HEAD" {
            stream.write_all(&body)?;
        }
        stream.flush()?;
    }
}

// ---------------------------------------------------------------------------
// Proxy server (SOCKS5 and HTTP CONNECT / absolute-form)
// ---------------------------------------------------------------------------

/// A proxy that only knows one hostname (`alias`), which it maps to `upstream`.
pub struct ProxyServer {
    port: u16,
    log: Arc<Mutex<Vec<String>>>,
}

impl ProxyServer {
    pub fn start(alias: &str, upstream: SocketAddr) -> Self {
        Self::spawn(Some((alias.to_string(), upstream)))
    }

    /// A proxy that resolves and connects to real hosts (for network tests).
    pub fn passthrough() -> Self {
        Self::spawn(None)
    }

    fn spawn(alias: Option<(String, SocketAddr)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let l = log.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (l, alias) = (l.clone(), alias.clone());
                thread::spawn(move || {
                    let _ = proxy_conn(stream, &alias, &l);
                });
            }
        });
        ProxyServer { port, log }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// One entry per proxied connection: `"socks5 host:port"`,
    /// `"connect host:port"` or `"forward host:port"`.
    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

fn proxy_conn(
    mut client: TcpStream,
    alias: &Option<(String, SocketAddr)>,
    log: &Mutex<Vec<String>>,
) -> std::io::Result<()> {
    let resolve = |host: &str, port: u16| -> Option<SocketAddr> {
        match alias {
            Some((name, upstream)) => (host == name).then_some(*upstream),
            None => {
                use std::net::ToSocketAddrs;
                (host, port).to_socket_addrs().ok()?.next()
            }
        }
    };

    let mut first = [0u8; 1];
    client.read_exact(&mut first)?;

    if first[0] == 0x05 {
        // SOCKS5 greeting: nmethods + methods; reply "no auth".
        let mut n = [0u8; 1];
        client.read_exact(&mut n)?;
        let mut methods = vec![0u8; n[0] as usize];
        client.read_exact(&mut methods)?;
        client.write_all(&[0x05, 0x00])?;

        // Request: VER CMD RSV ATYP
        let mut req = [0u8; 4];
        client.read_exact(&mut req)?;
        let host = match req[3] {
            0x01 => {
                let mut ip = [0u8; 4];
                client.read_exact(&mut ip)?;
                std::net::Ipv4Addr::from(ip).to_string()
            }
            0x03 => {
                let mut len = [0u8; 1];
                client.read_exact(&mut len)?;
                let mut name = vec![0u8; len[0] as usize];
                client.read_exact(&mut name)?;
                String::from_utf8_lossy(&name).to_string()
            }
            _ => return Ok(()),
        };
        let mut port = [0u8; 2];
        client.read_exact(&mut port)?;
        let port = u16::from_be_bytes(port);
        log.lock().unwrap().push(format!("socks5 {host}:{port}"));

        let Some(target) = resolve(&host, port) else {
            // 0x04: host unreachable
            client.write_all(&[0x05, 0x04, 0x00, 0x01, 0, 0, 0, 0, 0, 0])?;
            return Ok(());
        };
        let server = TcpStream::connect(target)?;
        client.write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0, 0])?;
        tunnel(client, server);
        return Ok(());
    }

    // HTTP proxy: read the request head.
    let mut head = first.to_vec();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        client.read_exact(&mut byte)?;
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head).to_string();
    let request_line = text.lines().next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");

    if method == "CONNECT" {
        log.lock().unwrap().push(format!("connect {target}"));
        let (host, port) = split_host_port(target, 443);
        let Some(addr) = resolve(host, port) else {
            client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")?;
            return Ok(());
        };
        let server = TcpStream::connect(addr)?;
        client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")?;
        tunnel(client, server);
    } else {
        // Absolute-form request, e.g. `GET http://host:port/path HTTP/1.1`.
        let authority = target
            .split_once("://")
            .map_or(target, |(_, rest)| rest.split('/').next().unwrap_or(""));
        log.lock().unwrap().push(format!("forward {authority}"));
        let (host, port) = split_host_port(authority, 80);
        let Some(addr) = resolve(host, port) else {
            client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")?;
            return Ok(());
        };
        let mut server = TcpStream::connect(addr)?;
        server.write_all(&head)?;
        tunnel(client, server);
    }
    Ok(())
}

fn split_host_port(authority: &str, default_port: u16) -> (&str, u16) {
    match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().unwrap_or(default_port)),
        None => (authority, default_port),
    }
}

fn tunnel(client: TcpStream, server: TcpStream) {
    let (mut c_read, mut s_write) = (client.try_clone().unwrap(), server.try_clone().unwrap());
    let up = thread::spawn(move || {
        let _ = std::io::copy(&mut c_read, &mut s_write);
        let _ = s_write.shutdown(Shutdown::Write);
    });
    let (mut s_read, mut c_write) = (server, client);
    let _ = std::io::copy(&mut s_read, &mut c_write);
    let _ = c_write.shutdown(Shutdown::Write);
    let _ = up.join();
}

// ---------------------------------------------------------------------------
// Stub cargo
// ---------------------------------------------------------------------------

/// A fake `cargo` on PATH that logs its arguments and "builds" a script
/// printing `built-from-source <args>`.
#[cfg(unix)]
pub struct StubCargo {
    dir: tempfile::TempDir,
}

#[cfg(unix)]
impl StubCargo {
    pub fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = r#"#!/bin/sh
root=""; prev=""; out=""
for a in "$@"; do
  if [ "$prev" = "--root" ]; then root="$a"; out="$out <tmp>"; else out="$out $a"; fi
  prev="$a"
done
echo "${out# }" >> "$STUB_CARGO_LOG"
mkdir -p "$root/bin"
printf '#!/bin/sh\necho built-from-source "$@"\n' > "$root/bin/tool"
chmod +x "$root/bin/tool"
"#;
        let path = dir.path().join("cargo");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(dir.path().join("calls.log"), "").unwrap();
        StubCargo { dir }
    }

    /// Put the stub first on PATH for `cmd`.
    pub fn apply(&self, mut cmd: assert_cmd::Command) -> assert_cmd::Command {
        let path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{path}", self.dir.path().display()))
            .env("STUB_CARGO_LOG", self.dir.path().join("calls.log"));
        cmd
    }

    /// Each invocation's arguments, with the temp `--root` shown as `<tmp>`.
    pub fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.path().join("calls.log"))
            .unwrap()
            .lines()
            .map(String::from)
            .collect()
    }
}
