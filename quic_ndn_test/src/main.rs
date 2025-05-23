// Standalone QUIC Transport Test for NDN
// 
// This is a minimal implementation of NDN over QUIC 
// using the Quinn crate to demonstrate core functionality

use std::sync::Arc;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::io::Cursor;

use tokio::io::AsyncWriteExt;
use bytes::{Bytes, BytesMut, BufMut};

use quinn::{ClientConfig, Connection, Endpoint, ServerConfig, TransportConfig};
use rustls::{Certificate, PrivateKey};

// Create self-signed certificate for testing
fn generate_self_signed_cert() -> Result<(Certificate, PrivateKey), Box<dyn std::error::Error>> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key = cert.serialize_private_key_der();
    let cert = cert.serialize_der()?;
    Ok((Certificate(cert), PrivateKey(key)))
}

// Simple Interest packet structure
struct Interest {
    name: String,
    nonce: u32,
}

impl Interest {
    fn new(name: &str) -> Self {
        let nonce = rand::random::<u32>();
        Self {
            name: name.to_string(),
            nonce,
        }
    }
    
    fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        
        // Add TLV type and length for Interest (0x05)
        buf.put_u8(0x05);
        
        // We'll calculate the length later
        let len_pos = buf.len();
        buf.put_u16(0); // Placeholder
        
        // Add name TLV (0x07)
        buf.put_u8(0x07);
        buf.put_u16(self.name.len() as u16);
        buf.put_slice(self.name.as_bytes());
        
        // Add nonce TLV (0x0A)
        buf.put_u8(0x0A);
        buf.put_u8(4); // 4 bytes
        buf.put_u32(self.nonce);
        
        // Update the length
        let interest_len = buf.len() - len_pos - 2;
        let mut cursor = Cursor::new(&mut buf[len_pos..len_pos+2]);
        cursor.get_mut().put_u16(interest_len as u16);
        
        buf.freeze()
    }
    
    fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        // Simple parsing for test purposes
        if bytes[0] != 0x05 {
            return Err("Not an Interest packet".into());
        }
        
        let mut name = String::new();
        let mut nonce = 0;
        
        let length = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
        let mut i = 3;
        
        while i < 3 + length {
            let tlv_type = bytes[i];
            i += 1;
            
            if i >= bytes.len() {
                break;
            }
            
            match tlv_type {
                0x07 => { // Name
                    let len = u16::from_be_bytes([bytes[i], bytes[i+1]]) as usize;
                    i += 2;
                    name = String::from_utf8_lossy(&bytes[i..i+len]).to_string();
                    i += len;
                },
                0x0A => { // Nonce
                    let len = bytes[i] as usize;
                    i += 1;
                    if len == 4 {
                        nonce = u32::from_be_bytes([bytes[i], bytes[i+1], bytes[i+2], bytes[i+3]]);
                    }
                    i += len;
                },
                _ => {
                    // Skip unknown TLV
                    let len = bytes[i] as usize;
                    i += 1 + len;
                }
            };
        }
        
        Ok(Interest { name, nonce })
    }
}

// Simple Data packet structure
struct Data {
    name: String,
    content: Bytes,
}

impl Data {
    fn new(name: &str, content: Bytes) -> Self {
        Self {
            name: name.to_string(),
            content,
        }
    }
    
    fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256 + self.content.len());
        
        // Add TLV type and length for Data (0x06)
        buf.put_u8(0x06);
        
        // We'll calculate the length later
        let len_pos = buf.len();
        buf.put_u16(0); // Placeholder
        
        // Add name TLV (0x07)
        buf.put_u8(0x07);
        buf.put_u16(self.name.len() as u16);
        buf.put_slice(self.name.as_bytes());
        
        // Add content TLV (0x15)
        buf.put_u8(0x15);
        buf.put_u16(self.content.len() as u16);
        buf.put_slice(&self.content);
        
        // Update the length
        let data_len = buf.len() - len_pos - 2;
        let mut cursor = Cursor::new(&mut buf[len_pos..len_pos+2]);
        cursor.get_mut().put_u16(data_len as u16);
        
        buf.freeze()
    }
    
    fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        // Simple parsing for test purposes
        if bytes[0] != 0x06 {
            return Err("Not a Data packet".into());
        }
        
        let mut name = String::new();
        let mut content = Bytes::new();
        
        let length = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
        let mut i = 3;
        
        while i < 3 + length {
            let tlv_type = bytes[i];
            i += 1;
            
            if i >= bytes.len() {
                break;
            }
            
            match tlv_type {
                0x07 => { // Name
                    let len = u16::from_be_bytes([bytes[i], bytes[i+1]]) as usize;
                    i += 2;
                    name = String::from_utf8_lossy(&bytes[i..i+len]).to_string();
                    i += len;
                },
                0x15 => { // Content
                    let len = u16::from_be_bytes([bytes[i], bytes[i+1]]) as usize;
                    i += 2;
                    content = Bytes::copy_from_slice(&bytes[i..i+len]);
                    i += len;
                },
                _ => {
                    // Skip unknown TLV
                    let len = bytes[i] as usize;
                    i += 1 + len;
                }
            };
        }
        
        Ok(Data { name, content })
    }
}

async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting QUIC NDN server...");
    
    // Generate self-signed certificate
    let (cert, key) = generate_self_signed_cert()?;
    
    // Create server config
    let mut server_config = ServerConfig::with_single_cert(vec![cert], key)?;
    let mut transport_config = TransportConfig::default();
    transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
    transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
    server_config.transport_config(Arc::new(transport_config));
    
    // Create server endpoint
    let addr = "127.0.0.1:9000".parse::<SocketAddr>()?;
    let endpoint = Endpoint::server(server_config, addr)?;
    println!("Server listening on {}", addr);
    
    // Accept connections
    while let Some(conn) = endpoint.accept().await {
        println!("Incoming connection from {:?}", conn.remote_address());
        
        // Handle the connection
        tokio::spawn(async move {
            match conn.await {
                Ok(connection) => {
                    println!("Connection established with {}", connection.remote_address());
                    
                    // Accept streams from this connection
                    while let Ok((send, recv)) = connection.accept_bi().await {
                        // Handle the stream
                        tokio::spawn(async move {
                            if let Err(e) = handle_stream(send, recv).await {
                                eprintln!("Stream error: {}", e);
                            }
                        });
                    }
                    
                    println!("Connection closed");
                },
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                }
            }
        });
    }
    
    Ok(())
}

async fn handle_stream(
    mut send: quinn::SendStream,
    mut recv: quinn::RecvStream,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read the Interest packet with 10MB size limit
    let buffer = recv.read_to_end(10_000_000).await?;
    
    // Parse the Interest
    let interest = Interest::from_bytes(&buffer)?;
    println!("Received Interest: {}", interest.name);
    
    // Create Data response
    let content = format!("Response data for {}", interest.name).into();
    let data = Data::new(&interest.name, content);
    
    // Send Data
    let data_bytes = data.to_bytes();
    send.write_all(&data_bytes).await?;
    send.finish().await?;
    
    println!("Sent Data response for {}", interest.name);
    Ok(())
}

async fn run_client(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting QUIC NDN client...");
    
    // Create client config
    let client_config = {
        let crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();
        ClientConfig::new(Arc::new(crypto))
    };
    
    // Create client endpoint
    let endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    
    // Connect to server
    let server_addr = "127.0.0.1:9000".parse::<SocketAddr>()?;
    println!("Connecting to {}...", server_addr);
    let connection = endpoint.connect_with(client_config, server_addr, "localhost")?
        .await?;
    println!("Connected to {}", server_addr);
    
    // Open a bi-directional stream
    let (mut send, mut recv) = connection.open_bi().await?;
    
    // Create Interest
    let interest = Interest::new(name);
    println!("Sending Interest for {}", interest.name);
    
    // Start time for RTT calculation
    let start_time = Instant::now();
    
    // Send Interest
    send.write_all(&interest.to_bytes()).await?;
    send.finish().await?;
    
    // Receive Data with 10MB size limit
    let buffer = recv.read_to_end(10_000_000).await?;
    
    // Calculate RTT
    let rtt = start_time.elapsed().as_millis();
    
    // Parse Data
    let data = Data::from_bytes(&buffer)?;
    println!("Received Data for {} with content: {}", data.name, String::from_utf8_lossy(&data.content));
    println!("RTT: {}ms", rtt);
    
    // Close connection
    connection.close(0u32.into(), b"done");
    
    Ok(())
}

// Certificate verification that skips actual verification (for testing only!)
struct SkipServerVerification;

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: quic_ndn_test [server|client] [name]");
        eprintln!("  server - Run in server mode");
        eprintln!("  client NAME - Run in client mode with NAME");
        return Ok(());
    }
    
    match args[1].as_str() {
        "server" => {
            run_server().await?;
        },
        "client" => {
            let name = if args.len() > 2 {
                &args[2]
            } else {
                "/test/data"
            };
            run_client(name).await?;
        },
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            eprintln!("Use 'server' or 'client'");
        }
    }
    
    Ok(())
}
