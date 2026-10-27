//! Explicit, authenticated loopback transport. No server in normal launches.
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};
pub struct Request {
    pub method: String,
    pub params: Value,
    pub reply: Sender<Value>,
}
pub struct Server {
    pub requests: Receiver<Request>,
    path: PathBuf,
    token: String,
}
impl Drop for Server {
    fn drop(&mut self) {
        // Only remove our own discovery file, never a replacement from another instance.
        if fs::read(&self.path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|v| v["token"].as_str() == Some(&self.token))
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}
pub fn start(path: PathBuf, ctx: egui::Context) -> io::Result<Server> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(io::Error::other)?;
    let token = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    file.write_all(
        serde_json::to_string(
            &json!({"host":"127.0.0.1","port":port,"token":token,"pid":std::process::id()}),
        )?
        .as_bytes(),
    )?;
    let (tx, requests) = mpsc::channel();
    let key = token.clone();
    std::thread::Builder::new()
        .name("Polymath agent control".into())
        .spawn(move || {
            // Serial connections bound resource use. Clients send one line then await their reply.
            for stream in listener.incoming().flatten() {
                let _ = serve(stream, &key, &tx, &ctx);
            }
        })?;
    Ok(Server {
        requests,
        path,
        token,
    })
}
fn serve(
    mut stream: TcpStream,
    token: &str,
    tx: &Sender<Request>,
    ctx: &egui::Context,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut line = String::new();
    BufReader::new((&stream).take(1_048_577)).read_line(&mut line)?;
    let v = if line.len() > 1_048_576 {
        json!({})
    } else {
        serde_json::from_str::<Value>(&line).unwrap_or(Value::Null)
    };
    let id = v.get("id").cloned().unwrap_or(Value::Null);
    let mut response = if v.get("token").and_then(Value::as_str) != Some(token) {
        error("authentication failed")
    } else if let Some(method) = v.get("method").and_then(Value::as_str) {
        let (reply, rx) = mpsc::channel();
        if tx
            .send(Request {
                method: method.into(),
                params: v.get("params").cloned().unwrap_or(json!({})),
                reply,
            })
            .is_err()
        {
            error("Polymath closed")
        } else {
            ctx.request_repaint();
            rx.recv_timeout(Duration::from_secs(45)).unwrap_or_else(|_|error("Polymath did not answer within 45 seconds; bring its window forward and inspect before retrying a mutation"))
        }
    } else {
        error("missing method")
    };
    response["id"] = id;
    writeln!(stream, "{response}")
}
pub fn ok(v: Value) -> Value {
    json!({"ok":true,"result":v})
}
pub fn error(e: impl std::fmt::Display) -> Value {
    json!({"ok":false,"error":e.to_string()})
}
