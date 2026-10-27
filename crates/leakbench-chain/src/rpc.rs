//! A minimal JSON-RPC client for a local node.
//!
//! Why not `reqwest`/`ureq`: this only ever talks to `127.0.0.1`, so a TLS stack
//! is dead weight, and being dependency-light is a feature for a tool that
//! claims to be auditable. HTTP/1.1 with `Content-Length` (and a chunked
//! decoder for nodes that stream) is enough.
//!
//! Remote nodes are deliberately unsupported: point an SSH tunnel at localhost
//! instead of sending your queries over the internet.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use serde_json::{json, Value};

use crate::ChainError;

#[derive(Debug, Clone)]
pub struct Rpc {
    host: String,
    port: u16,
    path: String,
    auth: Option<String>,
}

impl Rpc {
    /// Parse `http://user:pass@127.0.0.1:18232` or `127.0.0.1:18232`.
    pub fn parse(url: &str) -> Result<Self, ChainError> {
        let url = url.strip_prefix("http://").unwrap_or(url);
        let (hostport, path) = match url.split_once('/') {
            Some((h, p)) => (h, format!("/{p}")),
            None => (url, "/".to_string()),
        };
        let (auth, hostport) = match hostport.rsplit_once('@') {
            Some((a, hp)) => (Some(a.to_string()), hp),
            None => (None, hostport),
        };
        let (host, port) = match hostport.rsplit_once(':') {
            Some((h, p)) => (
                h.to_string(),
                p.parse::<u16>()
                    .map_err(|_| ChainError::Invalid(format!("bad port in {url:?}")))?,
            ),
            None => (hostport.to_string(), 8232),
        };
        if host.is_empty() {
            return Err(ChainError::Invalid(format!("bad node url {url:?}")));
        }
        Ok(Rpc {
            host,
            port,
            path,
            auth,
        })
    }

    pub fn endpoint(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value, ChainError> {
        let request = json!({
            "jsonrpc": "1.0",
            "id": "leakbench",
            "method": method,
            "params": params,
        });
        let body = serde_json::to_string(&request)?;
        let raw = self.post(&body)?;
        let value: Value = serde_json::from_slice(&raw).map_err(|e| {
            ChainError::Invalid(format!("node returned non-JSON for {method}: {e}"))
        })?;
        if let Some(err) = value.get("error").filter(|e| !e.is_null()) {
            return Err(ChainError::Rpc(format!("{method}: {err}")));
        }
        Ok(value.get("result").cloned().unwrap_or(Value::Null))
    }

    fn post(&self, body: &str) -> Result<Vec<u8>, ChainError> {
        let mut stream = TcpStream::connect((self.host.as_str(), self.port)).map_err(|e| {
            ChainError::Rpc(format!(
                "cannot reach {}:{} ({e}). Is the node running with RPC enabled?",
                self.host, self.port
            ))
        })?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;

        let mut head = format!(
            "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n",
            self.path,
            self.host,
            self.port,
            body.len()
        );
        if let Some(auth) = &self.auth {
            head.push_str(&format!("Authorization: Basic {}\r\n", base64(auth)));
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes())?;
        stream.write_all(body.as_bytes())?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut status_line = String::new();
        reader.read_line(&mut status_line)?;
        let status = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        let mut content_length: Option<usize> = None;
        let mut chunked = false;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(v) = lower.strip_prefix("content-length:") {
                content_length = v.trim().parse().ok();
            }
            if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
                chunked = true;
            }
        }

        let bytes = if let Some(n) = content_length {
            let mut buf = vec![0u8; n];
            reader.read_exact(&mut buf)?;
            buf
        } else if chunked {
            read_chunked(&mut reader)?
        } else {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf)?;
            buf
        };

        if status != 200 && status != 0 && bytes.is_empty() {
            return Err(ChainError::Rpc(format!("node returned HTTP {status}")));
        }
        Ok(bytes)
    }

    /// Current chain tip height.
    pub fn block_count(&self) -> Result<u64, ChainError> {
        let v = self.call("getblockcount", json!([]))?;
        v.as_u64()
            .ok_or_else(|| ChainError::Invalid("getblockcount returned no number".into()))
    }

    pub fn block_hash(&self, height: u64) -> Result<String, ChainError> {
        let v = self.call("getblockhash", json!([height]))?;
        v.as_str()
            .map(str::to_string)
            .ok_or_else(|| ChainError::Invalid("getblockhash returned no hash".into()))
    }

    /// Decoded transaction, as the node's own decoder sees it.
    ///
    /// `verbose = 1` is what we need: it carries the shield action counts and
    /// per-pool value balances that LeakBench analyzes.
    pub fn raw_transaction(&self, txid: &str) -> Result<Value, ChainError> {
        self.call("getrawtransaction", json!([txid, 1]))
    }

    pub fn block(&self, hash: &str, verbosity: u8) -> Result<Value, ChainError> {
        self.call("getblock", json!([hash, verbosity]))
    }

    /// True when the node answers `getblockcount`. Used by `leakbench doctor`.
    pub fn ping(&self) -> Result<u64, ChainError> {
        self.block_count()
    }
}

fn read_chunked(reader: &mut impl BufRead) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let size_hex = line.trim().split(';').next().unwrap_or("");
        let size = usize::from_str_radix(size_hex, 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        let mut buf = vec![0u8; size];
        reader.read_exact(&mut buf)?;
        out.extend_from_slice(&buf);
        let mut crlf = String::new();
        reader.read_line(&mut crlf)?;
    }
    Ok(out)
}

/// Base64 for the `Authorization: Basic` header. Twenty lines beats a crate
/// that pulls in a TLS stack.
pub fn base64(input: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18 & 63) as usize] as char);
        out.push(TABLE[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_endpoints() {
        let r = Rpc::parse("http://127.0.0.1:18232").unwrap();
        assert_eq!(r.endpoint(), "http://127.0.0.1:18232");
        let r = Rpc::parse("u:p@localhost:8232").unwrap();
        assert_eq!(r.auth.as_deref(), Some("u:p"));
        let r = Rpc::parse("127.0.0.1").unwrap();
        assert_eq!(r.port, 8232);
        assert!(Rpc::parse("http://:123").is_err());
    }

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(""), "");
        assert_eq!(base64("f"), "Zg==");
        assert_eq!(base64("fo"), "Zm8=");
        assert_eq!(base64("foo"), "Zm9v");
        assert_eq!(base64("user:pass"), "dXNlcjpwYXNz");
    }
}
