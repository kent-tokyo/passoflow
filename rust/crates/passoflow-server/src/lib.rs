//! Small, local-only HTTP bootstrap for the future Rust Web UI server.
//!
//! The initial boundary intentionally uses only the standard library. It is
//! not a general-purpose HTTP framework and does not yet expose scenario data
//! or execution control.

#![forbid(unsafe_code)]

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};

use serde_json::json;

/// A minimal local server for the Rust API migration bootstrap.
#[derive(Debug)]
pub struct Server {
    listener: TcpListener,
}

impl Server {
    /// Bind to the caller-selected address. Use `127.0.0.1:0` for a local
    /// ephemeral test port; callers should not pass a public interface.
    ///
    /// # Errors
    ///
    /// Returns the operating-system bind error when the address is unavailable.
    pub fn bind(address: &str) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(address)?,
        })
    }

    /// Return the address selected by the operating system.
    ///
    /// # Errors
    ///
    /// Returns the listener address error when the bound socket cannot report
    /// its local address.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Serve one request, then return. This is the intentionally small seam
    /// that the eventual async/local server can replace without changing the
    /// route contract.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when accepting, reading, or writing the request
    /// fails.
    pub fn serve_once(&self) -> io::Result<()> {
        let (stream, _) = self.listener.accept()?;
        handle_connection(stream)
    }
}

fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    let mut request = Vec::with_capacity(512);
    let mut chunk = [0_u8; 512];
    while request.len() < 8 * 1024 {
        let size = stream.read(&mut chunk)?;
        if size == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..size]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let request = String::from_utf8_lossy(&request);
    let request_line = request.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    let (status, body) = match (method, path) {
        ("GET", "/api/health") => (
            "200 OK",
            json!({"status": "ok", "contract": passoflow_core::CONTRACT_VERSION}),
        ),
        ("GET", "/api/version") => ("200 OK", json!({"version": env!("CARGO_PKG_VERSION")})),
        ("GET", _) => ("404 Not Found", json!({"detail": "not found"})),
        _ => (
            "405 Method Not Allowed",
            json!({"detail": "method not allowed"}),
        ),
    };
    let payload = serde_json::to_vec(&body).map_err(io::Error::other)?;
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    )?;
    stream.write_all(&payload)?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpStream};
    use std::thread;

    use super::Server;

    fn request(path: &str, method: &str) -> String {
        let server = Server::bind("127.0.0.1:0").expect("bind local server");
        let address = server.local_addr().expect("local address");
        let worker = thread::spawn(move || server.serve_once().expect("serve request"));
        let mut stream = TcpStream::connect(address).expect("connect local server");
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\n\r\n"
        )
        .expect("write request");
        stream
            .shutdown(Shutdown::Write)
            .expect("finish request body");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        worker.join().expect("server worker");
        response
    }

    #[test]
    fn serves_local_health_and_version_contracts() {
        let health = request("/api/health", "GET");
        assert!(health.starts_with("HTTP/1.1 200 OK"), "{health:?}");
        assert!(health.contains("\"status\":\"ok\""));
        assert!(health.contains("\"contract\":\"0.1\""));

        let health_with_query = request("/api/health?detail=1", "GET");
        assert!(health_with_query.starts_with("HTTP/1.1 200 OK"));

        let version = request("/api/version", "GET");
        assert!(version.starts_with("HTTP/1.1 200 OK"), "{version:?}");
        assert!(version.contains("\"version\":\"0.1.3\""));
    }

    #[test]
    fn rejects_unknown_routes_and_methods() {
        assert!(request("/api/scenarios", "GET").starts_with("HTTP/1.1 404 Not Found"));
        assert!(request("/api/health", "POST").starts_with("HTTP/1.1 405 Method Not Allowed"));
    }
}
