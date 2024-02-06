use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::net::TcpStream;
use std::str;
use url::Url;

use native_tls::{Identity, TlsConnector};
use native_tls::TlsStream;

use tungstenite::client as ws_client;
use tungstenite::WebSocket;

use serde_json::{self, Map};

#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(unix)]
use libc::{c_ulong, ioctl, FIONBIO};
use libc::{setsockopt, SOL_SOCKET, SO_KEEPALIVE};

//type TlsWebSocket = WebSocket<MaybeTlsStream<TcpStream>>;
type TlsWebSocket = WebSocket<TlsStream<TcpStream>>;

#[allow(dead_code)]    // don't complain that we do not use these fields
#[derive(Debug)]    // we use these fields when doing a Debug-print {:?}
struct LokiMsg {
    tenant: String,
    src_loki: String,
    src_host: String,
    src_log: String,
    nanotime: String,
    raw_data: Option<String>,
    struct_data: Option<String>,
}

struct LokiStreamReader {
    ws: TlsWebSocket,
    loki_id: String,
}

#[cfg(unix)]
fn set_non_blocking<T>(socket: &T, mut non_blocking: isize) -> std::io::Result<()> where T: AsRawFd {
    let fd = socket.as_raw_fd();
    unsafe {
        let result = ioctl(fd, FIONBIO, &mut non_blocking as *mut _ as *mut c_ulong);
        if result == -1 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

fn set_keepalive(stream: &TcpStream, value: u32) -> std::io::Result<()> {
    let fd = stream.as_raw_fd();
    unsafe {
        let result = setsockopt(
            fd, SOL_SOCKET, SO_KEEPALIVE,
            &value as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32);
        if result == -1 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

impl LokiStreamReader {
    fn handle_messages(&mut self) -> Result<(), Box<dyn Error>> {
        let mut total = 0;

        loop {
            let msg = self.ws.read()?;
            let data = msg.into_text()?;

            if data.is_empty() {
                eprintln!("EMPTY");
                continue;
            }

            let data: HashMap<String, serde_json::Value> = serde_json::from_str(&data)?;

            let dropped_entries = data.get("dropped_entries").and_then(|d| d.as_array());
            let mut handled_entries = 0;

            let streams = data.get("streams").and_then(|s| s.as_array()).ok_or("Invalid data")?;

            if ! (data.len() == 1 || (data.len() == 2 && dropped_entries.is_some())) {
                panic!("bad root data: {:?}", data.keys());
            }

            for stream in streams {
                let stream = stream.as_object().ok_or("Invalid data")?;
                let labels = stream.get("stream").and_then(|s| s.as_object()).ok_or("Invalid data")?;
                let values = stream.get("values").and_then(|v| v.as_array()).ok_or("Invalid data")?;
                if stream.len() != 2 {
                    panic!("bad streams-element: {:?}", stream);
                }

                for value in values {
                    handled_entries += 1;
                    let loki_msg = self.make_lokimsg(labels, value)?;
                    println!("{} {:?}", total, loki_msg);
                    println!();
                    total += 1;
                }
            }

            eprintln!("HANDLED {}", handled_entries);
            if let Some(dropped) = dropped_entries {
                eprintln!("DROPPED (!) {}", dropped.len());
            }
        }
    }

    fn extract_log_sources(&self, labels: &Map<String, serde_json::Value>) -> Result<(String, String, String), Box<dyn Error>> {
        let tenant = labels.get("tenant").and_then(|t| t.as_str()).ok_or("Invalid labels")?.to_string();
        let src_host = labels.get("host").and_then(|h| h.as_str()).ok_or("Invalid labels")?.to_string();

        let src_log = if let Some(filename) = labels.get("filename").and_then(|f| f.as_str()) {
            filename.to_string()
        } else if let Some(systemd_unit) = labels.get("systemd_unit").and_then(|s| s.as_str()) {
            systemd_unit.to_string()
        } else {
            return Err("Invalid labels".into());
        };

        Ok((tenant, src_host, src_log))
    }

    fn make_lokimsg(&self, labels: &Map<String, serde_json::Value>, values: &serde_json::Value) -> Result<LokiMsg, Box<dyn Error>> {
        let nanotime = values.get(0).and_then(|v| v.as_str()).ok_or("Invalid values")?.to_string();
        let data = values.get(1).and_then(|d| d.as_str()).ok_or("Invalid values")?.to_string();
        let src_loki = self.loki_id.clone();
        let (tenant, src_host, src_log) = self.extract_log_sources(labels)?;

        let (raw_data, struct_data) = if let Ok(struct_data) = serde_json::from_str::<serde_json::Value>(&data) {
            (None, Some(serde_json::to_string(&struct_data)?))
        } else {
            (Some(data.clone()), None)
        };

        Ok(LokiMsg {
            tenant,
            src_loki,
            src_host,
            src_log,
            nanotime,
            raw_data,
            struct_data,
        })
    }
}

fn ws_client_connect(websocket_url: &str) -> Result<TlsWebSocket, Box<dyn Error>> {
    let url = Url::parse(websocket_url).unwrap();
    let hostname = url.domain().unwrap();
    let port = url.port().unwrap_or(443);

    // This is needed. The from_pkcs8() does not grok our crt+key.
    // openssl pkcs12 -export -out loki_client.pfx -inkey loki_client.key -in loki_client.crt
    let mut client_pfx_file = File::open("examples/loki_client.pfx")?;
    let mut client_pfx_buf = Vec::new();
    client_pfx_file.read_to_end(&mut client_pfx_buf)?;
    drop(client_pfx_file);

    let identity = Identity::from_pkcs12(&client_pfx_buf, "")?;
    let connector = TlsConnector::builder().identity(identity).build()?;

    // Connect to the WebSocket server
    let tcp_stream: TcpStream = TcpStream::connect((hostname, port))?;
    set_keepalive(&tcp_stream, 1)?;         // not needed
    set_non_blocking(&tcp_stream, 0)?;      // not needed
    let tls_stream: TlsStream<TcpStream> = connector.connect(hostname, tcp_stream)?;

    let (ws, _response) = ws_client(websocket_url, tls_stream)?;
    //println!("response: {_response:?}");

    Ok(ws)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        let argv0 = &args[0];
        eprintln!("Usage: {argv0} hostname filter");
        std::process::exit(1);
    }

    let hostname = &args[1];
    let filter_ = &args[2];
    let filter_encoded = urlencoding::encode(filter_);

    let websocket_url = format!(
        "wss://{}/loki/api/v1/tail?limit=1&query={}&start=1707222222000000000",
        hostname, filter_encoded);

    let ws = ws_client_connect(&websocket_url)?;

    let loki_id = hostname.to_string();
    let mut rd = LokiStreamReader { ws, loki_id };

    rd.handle_messages()?;

    Ok(())
}
