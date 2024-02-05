use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_native_tls::native_tls::{Identity, TlsConnector};
use tokio_native_tls::TlsConnector as TokioTlsConnector;

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

    // This is needed. The from_pkcs8() does not grok our crt+key.
    // openssl pkcs12 -export -out loki_client.pfx -inkey loki_client.key -in loki_client.crt
    let mut client_pfx_file = File::open("examples/loki_client.pfx")?;
    let mut client_pfx_buf = Vec::new();
    client_pfx_file.read_to_end(&mut client_pfx_buf)?;

    let identity = Identity::from_pkcs12(&client_pfx_buf, "")?;
    let native_connector = TlsConnector::builder().identity(identity).build()?;
    let connector = TokioTlsConnector::from(native_connector);

    let stream = TcpStream::connect((hostname.clone(), 443)).await?;
    let mut stream = connector.connect(hostname, stream).await?;

    let request = format!(
        "GET /ready HTTP/1.1\r\nConnection: close\r\nHost: {}\r\n\r\n",
        hostname
    );

    stream.write_all(request.as_bytes()).await?;
    let mut res = vec![];
    stream.read_to_end(&mut res).await?;
    println!("{}", String::from_utf8_lossy(&res));

    Ok(())
}

