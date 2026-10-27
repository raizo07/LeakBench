//! Static + reports API server. No framework: ~120 lines of `tiny_http`, so
//! the demo binary has the same dependency count as the analysis library.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use leakbench_core::Report;
use tiny_http::{Header, Response, Server};

pub fn run(dir: PathBuf, static_dir: PathBuf, port: u16) -> anyhow::Result<()> {
    let server = Server::http(("127.0.0.1", port))
        .map_err(|e| anyhow::anyhow!("cannot bind 127.0.0.1:{port}: {e}"))?;
    let dir = Arc::new(dir);
    let static_dir = Arc::new(static_dir);

    println!("leakbench serve");
    println!("  reports   {}", dir.display());
    println!("  static    {}", static_dir.display());
    println!("  url       http://127.0.0.1:{port}");
    println!("  api       http://127.0.0.1:{port}/api/index.json");
    println!("\nctrl-c to stop\n");

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or("/").to_string();
        let response = route(&path, &dir, &static_dir);
        let _ = request.respond(response);
    }
    Ok(())
}

fn route(path: &str, dir: &Path, static_dir: &Path) -> Response<std::io::Cursor<Vec<u8>>> {
    match path {
        "/api/health" => {
            json(serde_json::json!({ "ok": true, "service": "leakbench" }).to_string())
        }
        "/api/index.json" => match build_index(dir) {
            Ok(v) => json(v),
            Err(e) => not_found(&format!("cannot read reports in {}: {e}", dir.display())),
        },
        p if p.starts_with("/api/reports/") => {
            let name = p.trim_start_matches("/api/reports/");
            match safe_name(name) {
                Some(name) => match std::fs::read(dir.join(&name)) {
                    Ok(bytes) => Response::from_data(bytes).with_header(header("application/json")),
                    Err(_) => not_found("report not found"),
                },
                None => not_found("invalid report name"),
            }
        }
        "/" => serve_file(&static_dir.join("index.html"), "index.html"),
        p => {
            let rel = p.trim_start_matches('/');
            let candidate = static_dir.join(rel);
            if candidate.is_file() {
                serve_file(&candidate, rel)
            } else if !rel.contains('.') {
                // SPA fallback: let the client-side router have it.
                serve_file(&static_dir.join("index.html"), "index.html")
            } else {
                not_found("not found")
            }
        }
    }
}

fn build_index(dir: &Path) -> std::io::Result<String> {
    let mut entries = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    files.sort();
    for path in files {
        if path.file_name().map(|n| n == "index.json").unwrap_or(false) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(report) = serde_json::from_slice::<Report>(&bytes) else {
            continue;
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        entries.push(serde_json::json!({
            "file": file,
            "id": file.trim_end_matches(".json"),
            "label": report.subject.label,
            "kind": report.subject.kind.as_str(),
            "description": report.subject.description,
            "score": report.score.value,
            "grade": report.score.grade,
            "verdict": report.verdict.as_str(),
            "fatal": report.fatal_count(),
            "findings": report.findings.len(),
            "by_severity": report.score.by_severity,
            "tx_count": report.subject.tx_count,
            "generated_at": report.generated_at,
        }));
    }
    Ok(serde_json::json!({ "reports": entries }).to_string())
}

fn safe_name(name: &str) -> Option<String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    if !name.ends_with(".json") {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return None;
    }
    Some(name.to_string())
}

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

fn header(mime: &str) -> Header {
    Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).expect("static header")
}

fn cors(response: Response<std::io::Cursor<Vec<u8>>>) -> Response<std::io::Cursor<Vec<u8>>> {
    response.with_header(
        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).expect("static header"),
    )
}

fn serve_file(path: &Path, name: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => cors(Response::from_data(bytes).with_header(header(mime_for(name)))),
        Err(_) => not_found(&format!(
            "{} not found — run `pnpm --dir web build` first",
            path.display()
        )),
    }
}

fn json(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    cors(Response::from_data(body.into_bytes()).with_header(header("application/json")))
}

fn not_found(message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = format!(
        "<!doctype html><meta charset=utf-8><body style=\"font:14px ui-monospace,monospace;\
         background:#0d0f12;color:#e6e9ef;padding:40px\"><h1>404</h1><p>{message}</p>"
    );
    cors(
        Response::from_data(body.into_bytes())
            .with_status_code(404)
            .with_header(header("text/html; charset=utf-8")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal() {
        assert!(safe_name("../etc/passwd").is_none());
        assert!(safe_name("a/b.json").is_none());
        assert!(safe_name("naive.json").is_some());
        assert!(safe_name("naive.txt").is_none());
    }

    #[test]
    fn mime_types() {
        assert_eq!(mime_for("index.html"), "text/html; charset=utf-8");
        assert_eq!(mime_for("a.css"), "text/css; charset=utf-8");
        assert_eq!(mime_for("x.unknown"), "application/octet-stream");
    }
}
