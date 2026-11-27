//! hot_paths: hand-rolled std::time benchmarks for verse-ls: cold start to initialize response, and a bounded loop of documentSymbol requests in one warm session. Prints ns/op per workload.
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

const MAX_FRAME_BYTES: usize = 1 << 20;
const COLD_START_REPS: usize = 10;
const WARMUP_CALLS: usize = 20;
const REQUESTS: usize = 200;
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
    fn recv_until(&mut self, needle: &str) -> Result<(), String> {
        loop {
            let frame = self.recv()?;
            if frame.contains(needle) {
                return Ok(());
            }
        }
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
    fn initialize(&mut self) -> Result<(), String> {
        self.send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":null,"rootUri":null,"capabilities":{}}}"#)?;
        self.recv_until("\"id\":1")?;
        self.send(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#)?;
        let open = format!(
            r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{FIXTURE_URI}","languageId":"verse","version":1,"text":{}}}}}}}"#,
            escape(FIXTURE_TEXT)
        );
        self.send(&open)
    }
    fn document_symbol(&mut self, id: u64) -> Result<(), String> {
        self.send(&format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"textDocument/documentSymbol","params":{{"textDocument":{{"uri":"{FIXTURE_URI}"}}}}}}"#
        ))?;
        self.recv_until(&format!("\"id\":{id}"))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
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

fn escape(text: &str) -> String {
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

fn main() {
    if let Err(error) = run() {
        eprintln!("hot_paths: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let program = server_binary();
    let start = Instant::now();
    for _ in 0..COLD_START_REPS {
        let mut session = spawn(&program)?;
        session.initialize()?;
    }
    let cold_ns = start.elapsed().as_nanos() / COLD_START_REPS as u128;
    println!("cold_start_initialize: {cold_ns} ns/op over {COLD_START_REPS} reps");

    let mut session = spawn(&program)?;
    session.initialize()?;
    for i in 0..WARMUP_CALLS {
        session.document_symbol(100 + i as u64)?;
    }
    let start = Instant::now();
    for i in 0..REQUESTS {
        session.document_symbol(1000 + i as u64)?;
    }
    let elapsed = start.elapsed();
    let per_ns = elapsed.as_nanos() / REQUESTS as u128;
    let ops_s = REQUESTS as f64 / elapsed.as_secs_f64();
    println!("document_symbol_request: {per_ns} ns/op, {ops_s:.1} ops/s over {REQUESTS} requests");
    Ok(())
}
