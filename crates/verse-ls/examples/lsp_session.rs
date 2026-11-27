//! lsp_session: spawn verse-ls, send initialize + initialized + didOpen on a tiny Verse fixture, then one representative documentSymbol request and shutdown. Expected output: the initialize result shape (capabilities + serverInfo), the documentSymbol response shape (rung 0 advertises sync only, so an error/null shape is the honest answer), and a clean exit line.
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const MAX_FRAME_BYTES: usize = 1 << 20;
const FIXTURE_URI: &str = "file:///tmp/verse_ls_fixture.verse";
const FIXTURE_TEXT: &str = "using { /Fortnite.com/Devices }\nhello_device := class(creative_device):\n    OnBegin<override>()<suspends>:void =\n        Print(\"hello\")\n";

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

fn spawn(program: &str) -> Result<Session, String> {
    let mut child = Command::new(program)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn {program}: {e}"))?;
    let stdin = child.stdin.take().ok_or("child stdin missing")?;
    let stdout = child.stdout.take().ok_or("child stdout missing")?;
    Ok(Session {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

impl Session {
    fn send(&mut self, body: &str) -> Result<(), String> {
        let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        self.stdin
            .write_all(frame.as_bytes())
            .map_err(|e| format!("write: {e}"))?;
        self.stdin.flush().map_err(|e| format!("flush: {e}"))
    }
    fn recv(&mut self) -> Result<String, String> {
        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            let n = self
                .stdout
                .read_line(&mut line)
                .map_err(|e| format!("read header: {e}"))?;
            if n == 0 {
                return Err("server closed stdout".to_string());
            }
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                content_length = value
                    .parse()
                    .map_err(|_| "bad Content-Length".to_string())?;
            }
        }
        if content_length == 0 || content_length > MAX_FRAME_BYTES {
            return Err(format!("frame length out of bound: {content_length}"));
        }
        let mut buf = vec![0u8; content_length];
        self.stdout
            .read_exact(&mut buf)
            .map_err(|e| format!("read body: {e}"))?;
        String::from_utf8(buf).map_err(|_| "frame not UTF-8".to_string())
    }
}

fn server_binary() -> String {
    if let Ok(path) = std::env::var("SERVER_BIN") {
        return path;
    }
    // Search ancestors of this executable for the server binary:
    // directly (target/<profile>/<bin>) or under primo's
    // build/<pkg>/<hash>/out/ layout, with a bounded scan.
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..8 {
            let Some(d) = dir else { break };
            let direct = d.join("verse-ls");
            if direct.is_file() {
                return direct.to_string_lossy().into_owned();
            }
            if let Some(found) = scan_for(&d, "verse-ls", 4) {
                return found;
            }
            dir = d.parent().map(|p| p.to_path_buf());
        }
    }
    "verse-ls".to_string()
}

fn scan_for(root: &std::path::Path, name: &str, _depth: usize) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    // Primo's cargo layout: <profile>/build/<crate>/<hash>/out/<bin>-<hash>.
    let build = root.join("build");
    let Ok(crates) = std::fs::read_dir(&build) else {
        return None;
    };
    for krate in crates.flatten() {
        if !krate.file_name().to_string_lossy().starts_with("verse-ls") {
            continue;
        }
        let Ok(hashes) = std::fs::read_dir(krate.path()) else {
            continue;
        };
        for hash in hashes.flatten() {
            // Skip unit-test artifacts: their fingerprint dir carries
            // output-test-bin-*, the real binary's carries output-bin-*.
            let fingerprint = hash.path().join("fingerprint");
            if let Ok(entries) = std::fs::read_dir(&fingerprint) {
                let names: Vec<String> = entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                if names.iter().any(|n| n.starts_with("output-test-"))
                    && !names.iter().any(|n| n.starts_with("output-bin-"))
                {
                    continue;
                }
            }
            let out = hash.path().join("out");
            let Ok(files) = std::fs::read_dir(&out) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                let file_name = file.file_name();
                let file_name = file_name.to_string_lossy();
                let hashed = file_name.len() > name.len()
                    && file_name.starts_with(name)
                    && file_name.as_bytes()[name.len()] == b'-'
                    && !file_name.ends_with(".d");
                if (file_name == name || hashed)
                    && file
                        .metadata()
                        .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
                {
                    return Some(path.to_string_lossy().into_owned());
                }
            }
        }
    }
    None
}

fn summarize(line: &str, bound: usize) -> String {
    if line.len() <= bound {
        line.to_string()
    } else {
        format!("{}... [{} bytes total]", &line[..bound], line.len())
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("lsp_session: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut session = spawn(&server_binary())?;
    session.send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#)?;
    // Skip any log notifications until the id=1 response arrives.
    loop {
        let frame = session.recv()?;
        if frame.contains("\"id\":1") {
            println!("initialize: {}", summarize(&frame, 400));
            break;
        }
    }
    session.send(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#)?;
    let open = format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{FIXTURE_URI}","languageId":"verse","version":1,"text":{}}}}}}}"#,
        serde_json_escape(FIXTURE_TEXT)
    );
    session.send(&open)?;
    session.send(&format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{{"textDocument":{{"uri":"{FIXTURE_URI}"}}}}}}"#
    ))?;
    loop {
        let frame = session.recv()?;
        if frame.contains("\"id\":2") {
            println!("documentSymbol: {}", summarize(&frame, 400));
            break;
        }
    }
    session.send(r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#)?;
    loop {
        let frame = session.recv()?;
        if frame.contains("\"id\":3") {
            println!("shutdown: {}", summarize(&frame, 200));
            break;
        }
    }
    session.send(r#"{"jsonrpc":"2.0","method":"exit"}"#)?;
    drop(session.stdin);
    let status = session.child.wait().map_err(|e| format!("wait: {e}"))?;
    println!("exit: {status}");
    Ok(())
}

fn serde_json_escape(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
