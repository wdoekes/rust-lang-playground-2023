use std::env;
use std::collections::HashMap;
use serde_json::Map;
use std::error::Error;
use std::future::Future;
use std::str;
use url::Url;

use tokio::io::{AsyncReadExt};
use tokio::fs::File;
use tokio::net::TcpStream;

use tokio_native_tls::native_tls::{Identity, TlsConnector};
use tokio_native_tls::TlsStream;
use tokio_native_tls::TlsConnector as TokioTlsConnector;

use anyhow::Result;
use fastwebsockets::FragmentCollector;
use fastwebsockets::handshake;
use http_body_util::Empty;
use hyper::{Request, body::Bytes, upgrade::Upgraded, header::{UPGRADE, CONNECTION}};
use hyper_util::rt::TokioIo;

use serde_json;


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
    ws: FragmentCollector<TokioIo<Upgraded>>,
    loki_id: String,
}

impl LokiStreamReader {
    async fn handle_messages(&mut self) -> Result<(), Box<dyn Error>> {
        let mut total = 0;

        loop {
            let frame = self.ws.read_frame().await?;
            let data: String;
            match frame.opcode {
                fastwebsockets::OpCode::Text => {
                    data = String::from_utf8_lossy(&frame.payload).to_string();
                }
                _ => {
                    panic!("got unexpected opcode {:?}", frame.opcode);
                }
            }

            if data.is_empty() {
                break;
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
                    total += 1
                }
            }

            eprintln!("HANDLED {}", handled_entries);
            if let Some(dropped) = dropped_entries {
                eprintln!("DROPPED (!) {}", dropped.len());
            }
        }

        Ok(())
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

async fn ws_client_connect(websocket_url: &str) -> Result<FragmentCollector<TokioIo<Upgraded>>, Box<dyn Error>> {
    let url = Url::parse(websocket_url).unwrap();
    let hostname = url.domain().unwrap();
    let port = url.port().unwrap_or(443);

    // This is needed. The from_pkcs8() does not grok our crt+key.
    // openssl pkcs12 -export -out loki_client.pfx -inkey loki_client.key -in loki_client.crt
    let mut client_pfx_file = File::open("examples/loki_client.pfx").await?;
    let mut client_pfx_buf = Vec::new();
    client_pfx_file.read_to_end(&mut client_pfx_buf).await?;
    drop(client_pfx_file);

    let identity = Identity::from_pkcs12(&client_pfx_buf, "")?;
    let native_connector = TlsConnector::builder().identity(identity).build()?;
    let connector = TokioTlsConnector::from(native_connector);

    // Connect to the WebSocket server
    let tcp_stream: TcpStream = TcpStream::connect((hostname, port)).await?;
    let tls_stream: TlsStream<TcpStream> = connector.connect(hostname, tcp_stream).await?;

    let req = Request::builder()
        .method("GET")
        .uri(websocket_url)
        .header("Host", hostname)
        .header(UPGRADE, "websocket")
        .header(CONNECTION, "upgrade")
        .header(
            "Sec-WebSocket-Key",
            fastwebsockets::handshake::generate_key(),
            )
        .header("Sec-WebSocket-Version", "13")
        .body(Empty::<Bytes>::new())?;

    let (ws, _response) = handshake::client(&SpawnExecutor, req, tls_stream).await?;

    Ok(FragmentCollector::new(ws))
}

// Tie hyper's executor to tokio runtime
struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
Fut: Future + Send + 'static,
Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        let argv0 = &args[0];
        eprintln!("Usage: {argv0} hostname filter");
        std::process::exit(1);
    }

    let hostname = &args[1];
    let filter_ = &args[2];
    let filter_encoded = urlencoding::encode(filter_);

    let websocket_url = format!("wss://{}/loki/api/v1/tail?limit=1&query={}&start=0", hostname, filter_encoded);

    let ws = ws_client_connect(&websocket_url).await?;

    let loki_id = hostname.to_string();
    let mut rd = LokiStreamReader { ws, loki_id };

    rd.handle_messages().await?;

    Ok(())
}
