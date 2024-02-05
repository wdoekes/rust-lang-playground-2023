use std::env;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::net::{TcpStream, Shutdown};
use std::str;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use native_tls::{TlsConnector};
use url::Url;
use tungstenite::{connect, Message};

#[derive(Debug)]
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
    ws: tungstenite::WebSocket<TcpStream>,
    loki_id: String,
}

impl LokiStreamReader {
    fn messages(&mut self) -> Result<(), Box<dyn Error>> {
        let data_keys = vec!["streams".to_string()];
        let stream_keys = vec!["stream".to_string(), "values".to_string()];

        loop {
            let msg = self.ws.read_message()?;
            let data = msg.into_text()?;
            if data.is_empty() {
                break;
            }

            let data: HashMap<String, serde_json::Value> = serde_json::from_str(&data)?;

            let dropped_entries = data.get("dropped_entries").and_then(|d| d.as_array());
            let mut handled_entries = 0;

            let streams = data.get("streams").and_then(|s| s.as_array()).ok_or("Invalid data")?;
            for stream in streams {
                let stream = stream.as_object().ok_or("Invalid data")?;
                let labels = stream.get("stream").and_then(|s| s.as_object()).ok_or("Invalid data")?;
                let values = stream.get("values").and_then(|v| v.as_array()).ok_or("Invalid data")?;

                for value in values {
                    handled_entries += 1;
                    let loki_msg = self.make_lokimsg(labels, value)?;
                    println!("{:?}", loki_msg);
                    println!();
                }
            }

            eprintln!("HANDLED {}", handled_entries);
            if let Some(dropped) = dropped_entries {
                eprintln!("DROPPED {}", dropped.len());
            }
        }

        Ok(())
    }

    fn extract_log_sources(&self, labels: &HashMap<String, serde_json::Value>) -> Result<(String, String, String), Box<dyn Error>> {
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

    fn make_lokimsg(&self, labels: &HashMap<String, serde_json::Value>, values: &serde_json::Value) -> Result<LokiMsg, Box<dyn Error>> {
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

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        writeln!(io::stderr(), "Usage: {} hostname filter", args[0])?;
        std::process::exit(1);
    }

    let hostname = &args[1];
    let filter_ = &args[2];
    let filter_encoded = urlencoding::encode(filter_);

    let websocket_url = format!("wss://{}/loki/api/v1/tail?limit=1&query={}&start=0", hostname, filter_encoded);

    let mut tls_connector = TlsConnector::new()?;
    let stream = TcpStream::connect(format!("{}:443", hostname))?;
    let domain = webpki::DNSNameRef::try_from_ascii_str(hostname)?;
    let tls_stream = tls_connector.connect(domain, stream)?;

    let (mut ws, _) = connect(websocket_url, Some(tungstenite::protocol::Role::Client), Some(tls_stream))?;
    
    let loki_id = hostname.to_string();
    let mut rd = LokiStreamReader { ws, loki_id };

    rd.messages()?;

    Ok(())
}
