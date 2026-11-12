//! The local event receiver: a loopback TCP listener on its own thread.
//!
//! Security decisions (see docs/claude-code-integration.md → Transport):
//!
//! - Binds `127.0.0.1` only, on an OS-assigned port (no fixed port to
//!   collide with, nothing reachable from the network).
//! - Every message must start with the per-installation token from
//!   `receiver.json` (user-only file). The token is compared in constant
//!   time *before* the payload is parsed.
//! - It is not HTTP: no CORS, no browser can form a valid message, and an
//!   HTTP request is rejected as malformed.
//! - One message per connection, at most 4 KiB, read with short timeouts.
//! - Two token buckets: connections, and authenticated events.
//! - The only effect of a message is a new activity event. Nothing is ever
//!   executed, opened or written on a client's behalf.

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use super::protocol::{self, ReceiverInfo, Rejection, WireEvent, MAX_MESSAGE};
use crate::services::paths;

const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// A token bucket: `capacity` burst, refilled at `per_second`.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    capacity: f64,
    per_second: f64,
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(capacity: u32, per_second: u32, now: Instant) -> Self {
        Self {
            capacity: f64::from(capacity),
            per_second: f64::from(per_second),
            tokens: f64::from(capacity),
            last: now,
        }
    }

    pub fn allow(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.per_second).min(self.capacity);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Accepts a validated event, or refuses it (e.g. the app is shutting down).
pub type Sink = Box<dyn Fn(WireEvent) -> Result<(), Rejection> + Send>;

pub struct Receiver {
    stop: Arc<AtomicBool>,
    port: u16,
    info_path: PathBuf,
    thread: Option<JoinHandle<()>>,
}

impl Receiver {
    /// Binds, publishes `receiver.json` and starts the listener thread. The
    /// token is reused across runs (per installation) unless the file is
    /// missing or invalid.
    pub fn start(dir: &Path, sink: Sink) -> std::io::Result<Self> {
        paths::ensure_private_dir(dir)?;
        let token = match protocol::read_info(dir) {
            Some(info) => info.token,
            None => protocol::new_token()?,
        };
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        let port = listener.local_addr()?.port();
        let info = ReceiverInfo {
            version: 1,
            port,
            token: token.clone(),
        };
        let info_path = protocol::info_path(dir);
        let json = serde_json::to_vec(&info).map_err(std::io::Error::other)?;
        paths::write_atomic(&info_path, &json)?;

        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("islandy-receiver".into())
                .spawn(move || serve(&listener, &token, &stop, &sink))?
        };
        Ok(Self {
            stop,
            port,
            info_path,
            thread: Some(thread),
        })
    }

    #[cfg(test)]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stops accepting, joins the thread and removes `receiver.json` (only
    /// if it still describes this listener).
    pub fn shutdown(&mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        self.stop.store(true, Ordering::SeqCst);
        // Wake the blocking accept().
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
            Duration::from_millis(200),
        );
        let _ = thread.join();
        let ours = std::fs::read(&self.info_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<ReceiverInfo>(&b).ok())
            .is_some_and(|info| info.port == self.port);
        if ours {
            let _ = std::fs::remove_file(&self.info_path);
        }
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn serve(listener: &TcpListener, token: &str, stop: &AtomicBool, sink: &Sink) {
    let start = Instant::now();
    let mut connections = RateLimiter::new(100, 50, start);
    let mut events = RateLimiter::new(60, 20, start);
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        if !connections.allow(Instant::now()) {
            let _ = stream.shutdown(Shutdown::Both);
            continue;
        }
        handle(stream, token, &mut events, sink);
    }
}

fn handle(mut stream: TcpStream, token: &str, events: &mut RateLimiter, sink: &Sink) {
    // Bound to loopback, so peers are local; check anyway.
    if !stream.peer_addr().is_ok_and(|a| a.ip().is_loopback()) {
        return;
    }
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(READ_TIMEOUT));
    let result = read_message(&mut stream)
        .and_then(|message| protocol::decode(&message, token))
        .and_then(|event| {
            if !events.allow(Instant::now()) {
                return Err(Rejection::RateLimited);
            }
            match event {
                WireEvent::Ping => Ok(()),
                event => sink(event),
            }
        });
    let reply = match result {
        Ok(()) => "ok\n".to_owned(),
        Err(rejection) => {
            #[cfg(debug_assertions)]
            crate::log_warn!(
                "receiver",
                "receiver rejected a message: {}",
                rejection.code()
            );
            format!("error {}\n", rejection.code())
        }
    };
    let _ = stream.write_all(reply.as_bytes());
    let _ = stream.shutdown(Shutdown::Both);
}

/// Reads until the second newline (header + one JSON line), EOF, the size
/// limit or the timeout, whichever comes first.
fn read_message(stream: &mut TcpStream) -> Result<Vec<u8>, Rejection> {
    let mut buffer = Vec::with_capacity(512);
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                if buffer.len() > MAX_MESSAGE {
                    return Err(Rejection::TooLarge);
                }
                if buffer.iter().filter(|b| **b == b'\n').count() >= 2 {
                    break;
                }
            }
            Err(_) => return Err(Rejection::Malformed),
        }
    }
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::services::activity::protocol::{
        send, session_key, ClaudeHook, ClaudeWire, SendError,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("islandy-receiver-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn stop_event() -> WireEvent {
        WireEvent::Claude(ClaudeWire {
            event: ClaudeHook::Stop,
            session: session_key("s"),
            project: Some("Islandy".into()),
            tool: None,
            notification: None,
            error: None,
            subagent: false,
        })
    }

    fn start(dir: &Path) -> (Receiver, mpsc::Receiver<WireEvent>) {
        let (tx, rx) = mpsc::channel();
        let receiver = Receiver::start(
            dir,
            Box::new(move |e| tx.send(e).map_err(|_| Rejection::Unavailable)),
        )
        .unwrap();
        (receiver, rx)
    }

    fn raw(port: u16, bytes: &[u8]) -> String {
        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let _ = s.write_all(bytes);
        let mut reply = String::new();
        let _ = s.read_to_string(&mut reply);
        reply
    }

    #[test]
    fn accepts_authenticated_events_and_rejects_everything_else() {
        let dir = temp_dir("auth");
        let (mut receiver, rx) = start(&dir);
        let info = protocol::read_info(&dir).expect("connection file published");
        assert_eq!(info.port, receiver.port());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(protocol::info_path(&dir))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "connection file is private");
        }

        send(&dir, &stop_event()).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            stop_event()
        );
        send(&dir, &WireEvent::Ping).unwrap();

        // Wrong token.
        let forged = protocol::encode(&"0".repeat(64), &stop_event()).unwrap();
        assert_eq!(raw(info.port, &forged), "error auth\n");
        // No token at all / an HTTP request from a browser.
        assert_eq!(
            raw(info.port, b"{\"kind\":\"ping\"}\n"),
            "error malformed\n"
        );
        let http = b"POST / HTTP/1.1\r\nOrigin: http://evil.example\r\n\r\n{}\n";
        assert_eq!(raw(info.port, http), "error malformed\n");
        // Oversized.
        let big = format!("ISLANDY1 {}\n{}\n", info.token, "x".repeat(MAX_MESSAGE));
        assert_eq!(raw(info.port, big.as_bytes()), "error size\n");
        // Valid token, invalid payload.
        let bad = format!(
            "ISLANDY1 {}\n{{\"kind\":\"shell\",\"command\":\"x\"}}\n",
            info.token
        );
        assert_eq!(raw(info.port, bad.as_bytes()), "error invalid\n");
        // A client that connects and says nothing times out.
        let silent = TcpStream::connect((Ipv4Addr::LOCALHOST, info.port)).unwrap();
        std::thread::sleep(READ_TIMEOUT + Duration::from_millis(100));
        drop(silent);
        send(&dir, &stop_event()).unwrap();

        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());
        assert!(
            rx.try_recv().is_err(),
            "rejected messages never reach the engine"
        );

        receiver.shutdown();
        assert!(
            protocol::read_info(&dir).is_none(),
            "connection file removed on exit"
        );
        assert_eq!(send(&dir, &stop_event()), Err(SendError::NotRunning));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keeps_the_token_across_restarts_and_survives_a_dead_port() {
        let dir = temp_dir("restart");
        let (mut first, _rx) = start(&dir);
        let token = protocol::read_info(&dir).unwrap().token;
        let port = first.port();
        // Simulate a crash: the file stays but nothing listens.
        first.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, port));
        first.thread.take().unwrap().join().unwrap();
        std::mem::forget(first);
        let started = Instant::now();
        assert_eq!(send(&dir, &stop_event()), Err(SendError::Unreachable));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "bounded when Islandy is gone"
        );

        let (mut second, rx) = start(&dir);
        assert_eq!(protocol::read_info(&dir).unwrap().token, token);
        send(&dir, &stop_event()).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());
        second.shutdown();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rate_limits_bursts() {
        let t0 = Instant::now();
        let mut limiter = RateLimiter::new(3, 2, t0);
        assert!((0..3).all(|_| limiter.allow(t0)));
        assert!(!limiter.allow(t0));
        assert!(limiter.allow(t0 + Duration::from_millis(500)));
        assert!(!limiter.allow(t0 + Duration::from_millis(500)));
        // Refill is capped at capacity.
        let later = t0 + Duration::from_secs(60);
        assert!((0..3).all(|_| limiter.allow(later)));
        assert!(!limiter.allow(later));
    }

    #[test]
    fn rejects_floods_of_authenticated_events() {
        let dir = temp_dir("flood");
        let (mut receiver, rx) = start(&dir);
        let mut rejected = 0;
        for _ in 0..90 {
            match send(&dir, &stop_event()) {
                Ok(()) => {}
                Err(SendError::Rejected(code)) => {
                    assert_eq!(code, "rate");
                    rejected += 1;
                }
                Err(e) => panic!("{e:?}"),
            }
        }
        assert!(rejected > 0, "a burst beyond the bucket is refused");
        assert!(rx.try_iter().count() < 90);
        receiver.shutdown();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
