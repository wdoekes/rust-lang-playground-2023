use std::env;
use std::error::Error;
use std::fs::File;
use std::future::Future;
use std::io::Read;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::native_tls::{Identity, TlsConnector};
use tokio_native_tls::TlsConnector as TokioTlsConnector;

use url::Url;

use anyhow::Result;
use fastwebsockets::FragmentCollector;
use fastwebsockets::handshake;
use http_body_util::Empty;
use hyper::{Request, body::Bytes, upgrade::Upgraded, header::{UPGRADE, CONNECTION}};
use hyper_util::rt::TokioIo;

async fn ws_client_connect(websocket_url: &str) -> Result<FragmentCollector<TokioIo<Upgraded>>, Box<dyn Error>> {
    // This is needed. The from_pkcs8() does not grok our crt+key.
    // openssl pkcs12 -export -out loki_client.pfx -inkey loki_client.key -in loki_client.crt
    let mut client_pfx_file = File::open("examples/loki_client.pfx")?;
    let mut client_pfx_buf = Vec::new();
    client_pfx_file.read_to_end(&mut client_pfx_buf)?;
    drop(client_pfx_file);

    let identity = Identity::from_pkcs12(&client_pfx_buf, "")?;
    let native_connector = TlsConnector::builder().identity(identity).build()?;
    let connector = TokioTlsConnector::from(native_connector);

    let url = Url::parse(websocket_url).unwrap();

    let hostname = url.domain().unwrap();
    let port = url.port().unwrap_or(443);

    // Connect to the WebSocket server
    let stream = TcpStream::connect((hostname, port)).await?;
    let mut stream = connector.connect(hostname, stream).await?;

    let request = format!(
        "GET /ready HTTP/1.1\r\nConnection: keep-alive\r\nHost: {}\r\n\r\n",
        hostname
    );

    stream.write_all(request.as_bytes()).await?;
    let mut res = [0; 8192];
    let mut len = 0;
    while len == 0 {
        len = stream.read(&mut res).await?;
    }
    println!("{len}");
    println!("{}", String::from_utf8_lossy(&res));

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

    let (ws, _) = handshake::client(&SpawnExecutor, req, stream).await?;

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
    let filter = &args[2];
    let filter_encoded = urlencoding::encode(filter);
    println!("hostname: {hostname}, filter: {filter_encoded}");

    let websocket_url = format!("wss://{}/loki/api/v1/tail?limit=1&query={}&start=0", hostname, filter_encoded);
    let mut ws = ws_client_connect(&websocket_url).await?;

    println!("reading..");
    let frame = ws.read_frame().await?;
    assert!(frame.fin);
    println!("read..");
// 
//     // Access the payload
//     match frame.opcode {
//         fastwebsockets::OpCode::Continuation => {
//             println!("FIXME");
//         }
//         fastwebsockets::OpCode::Binary => {
//             println!("Binary payload: {:?}", frame.payload);
//         }
//         fastwebsockets::OpCode::Text => {
//      //     println!("{}", String::from_utf8_lossy(&res));
//             println!("Text payload: {:?}", frame.payload);
//         }
//         fastwebsockets::OpCode::Close => {
//             println!("FIXME");
//         }
//         fastwebsockets::OpCode::Ping => {
//             println!("FIXME");
//         }
//         fastwebsockets::OpCode::Pong => {
//             println!("FIXME");
//         }
//     }

    Ok(())
}

