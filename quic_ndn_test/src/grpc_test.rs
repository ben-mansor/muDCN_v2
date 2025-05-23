// QUIC-gRPC Integration Test
//
// A simplified version of the integration test that demonstrates
// gRPC handling Interest requests and forwarding them over QUIC

use std::sync::Arc;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::io::Cursor;

use tokio::sync::RwLock;
use tokio::time::sleep;
use tokio::io::AsyncWriteExt;
use bytes::{Bytes, BytesMut, BufMut};

use tonic::{transport::Server, Request, Response, Status};
use quinn::{ClientConfig, Connection, Endpoint, ServerConfig, TransportConfig};
use rustls::{Certificate, PrivateKey, client::ServerCertVerifier, Error as RustlsError};

// gRPC proto definitions (simplified for standalone test)
pub mod ndn_service {
    tonic::include_proto!("ndn");
}

// Certificate verification that skips actual verification (for testing only!)
struct SkipServerVerification;

impl ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, RustlsError> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

// Self-signed certificate generation for QUIC
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

// QUIC transport for forwarding Interest/Data packets
struct QuicTransport {
    endpoint: Endpoint,
    connections: Arc<RwLock<std::collections::HashMap<String, Connection>>>,
}

impl QuicTransport {
    async fn new(bind_addr: &str, port: u16) -> Result<Self, Box<dyn std::error::Error>> {
        // Generate self-signed certificate
        let (cert, key) = generate_self_signed_cert()?;
        
        // Create server config
        let mut server_config = ServerConfig::with_single_cert(vec![cert], key)?;
        let mut transport_config = TransportConfig::default();
        transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
        transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
        server_config.transport_config(Arc::new(transport_config));
        
        // Create endpoint
        let addr = format!("{}:{}", bind_addr, port).parse::<SocketAddr>()?;
        let endpoint = Endpoint::server(server_config, addr)?;
        
        Ok(Self {
            endpoint,
            connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }
    
    async fn connect(&self, remote_addr: &str, port: u16) -> Result<String, Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", remote_addr, port).parse::<SocketAddr>()?;
        
        // Create client config
        let client_config = {
            let crypto = rustls::ClientConfig::builder()
                .with_safe_defaults()
                .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
                .with_no_client_auth();
            ClientConfig::new(Arc::new(crypto))
        };
        
        // Connect to server
        let connecting = self.endpoint.connect_with(client_config, addr, "localhost")?;
        let connection = connecting.await?;
        
        // Create a connection ID
        let connection_id = format!("conn-{}-{}", remote_addr, port);
        
        // Store connection
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id.clone(), connection);
        }
        
        Ok(connection_id)
    }
    
    async fn send_interest(&self, connection_id: &str, interest: Interest) 
        -> Result<Data, Box<dyn std::error::Error>> {
        
        // Get connection
        let connection = {
            let connections = self.connections.read().await;
            if let Some(conn) = connections.get(connection_id) {
                conn.clone()
            } else {
                return Err(format!("Connection {} not found", connection_id).into());
            }
        };
        
        // Open a bi-directional stream
        let (mut send, recv) = connection.open_bi().await?;
        
        // Start time for RTT calculation
        let start_time = Instant::now();
        
        // Send Interest
        send.write_all(&interest.to_bytes()).await?;
        send.finish().await?;
        
        // Receive Data
        let buffer = recv.read_to_end(10_000_000).await?;
        
        // Calculate RTT
        let rtt = start_time.elapsed().as_millis() as u64;
        
        // Parse Data
        let data = Data::from_bytes(&buffer)?;
        println!("Received Data for {}, RTT: {}ms", data.name, rtt);
        
        Ok(data)
    }
}

// gRPC server for NDN router
struct NdnRouterService {
    quic_transport: Arc<QuicTransport>,
}

#[tonic::async_trait]
impl ndn_service::ndn_router_server::NdnRouter for NdnRouterService {
    async fn send_interest(
        &self,
        request: Request<ndn_service::InterestRequest>,
    ) -> Result<Response<ndn_service::DataResponse>, Status> {
        let req = request.into_inner();
        println!("gRPC received Interest for: {}", req.name);
        
        // Get or create QUIC connection
        let connection_id = match self.quic_transport.connections.read().await.get(&req.remote_server) {
            Some(_) => req.remote_server.clone(),
            None => {
                // Parse remote server and port
                let parts: Vec<&str> = req.remote_server.split(":").collect();
                if parts.len() != 2 {
                    return Err(Status::invalid_argument("Invalid remote server format, expected host:port"));
                }
                
                let host = parts[0];
                let port = parts[1].parse::<u16>().map_err(|_| Status::invalid_argument("Invalid port"))?;
                
                // Connect to the remote server
                match self.quic_transport.connect(host, port).await {
                    Ok(conn_id) => conn_id,
                    Err(e) => return Err(Status::internal(format!("Failed to connect to remote server: {}", e))),
                }
            }
        };
        
        // Create and send Interest via QUIC
        let interest = Interest::new(&req.name);
        
        match self.quic_transport.send_interest(&connection_id, interest).await {
            Ok(data) => {
                // Create gRPC response
                let response = ndn_service::DataResponse {
                    name: data.name,
                    content: data.content.to_vec(),
                    success: true,
                    error_message: String::new(),
                };
                
                Ok(Response::new(response))
            },
            Err(e) => {
                Err(Status::internal(format!("Failed to send Interest: {}", e)))
            }
        }
    }
}

// Start a QUIC server for testing
async fn run_quic_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    // Generate self-signed certificate
    let (cert, key) = generate_self_signed_cert()?;
    
    // Create server config
    let mut server_config = ServerConfig::with_single_cert(vec![cert], key)?;
    let mut transport_config = TransportConfig::default();
    transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
    transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
    server_config.transport_config(Arc::new(transport_config));
    
    // Create server endpoint
    let addr = format!("127.0.0.1:{}", port).parse::<SocketAddr>()?;
    let endpoint = Endpoint::server(server_config, addr)?;
    
    println!("QUIC server listening on {}", addr);
    
    // Accept connections
    while let Some(conn) = endpoint.accept().await {
        println!("QUIC server received new connection from {:?}", conn.remote_address());
        
        // Handle connection
        tokio::spawn(async move {
            match conn.await {
                Ok(connection) => {
                    // Accept bi-directional streams
                    while let Ok((send, recv)) = connection.accept_bi().await {
                        tokio::spawn(async move {
                            if let Err(e) = handle_stream(send, recv).await {
                                eprintln!("Error handling stream: {}", e);
                            }
                        });
                    }
                },
                Err(e) => {
                    eprintln!("Connection failed: {}", e);
                }
            }
        });
    }
    
    Ok(())
}

// Handle QUIC stream (Interest/Data exchange)
async fn handle_stream(
    mut send: quinn::SendStream,
    recv: quinn::RecvStream,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read Interest
    let buffer = recv.read_to_end(10_000_000).await?;
    
    // Parse Interest
    let interest = Interest::from_bytes(&buffer)?;
    println!("QUIC server received Interest: {}", interest.name);
    
    // Create Data response
    let content = format!("This is test data for {}", interest.name).into();
    let data = Data::new(&interest.name, content);
    
    // Send Data
    send.write_all(&data.to_bytes()).await?;
    send.finish().await?;
    
    println!("QUIC server sent Data response for {}", interest.name);
    
    Ok(())
}

// Start gRPC server for testing
async fn run_grpc_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    // Create QUIC transport
    let quic_transport = Arc::new(QuicTransport::new("127.0.0.1", 0).await?);
    
    // Create gRPC service
    let router_service = NdnRouterService {
        quic_transport,
    };
    
    // Create gRPC server
    let addr = format!("127.0.0.1:{}", port).parse::<SocketAddr>()?;
    println!("gRPC server listening on {}", addr);
    
    Server::builder()
        .add_service(ndn_service::ndn_router_server::NdnRouterServer::new(router_service))
        .serve(addr)
        .await?;
    
    Ok(())
}

// Main test function
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Constants for testing
    const GRPC_PORT: u16 = 9090;
    const QUIC_SERVER_PORT: u16 = 9000;
    
    // Start QUIC server in background
    let quic_server_handle = tokio::spawn(async move {
        if let Err(e) = run_quic_server(QUIC_SERVER_PORT).await {
            eprintln!("QUIC server error: {}", e);
        }
    });
    
    // Wait for QUIC server to start
    sleep(Duration::from_millis(500)).await;
    
    // Start gRPC server in background
    let grpc_server_handle = tokio::spawn(async move {
        if let Err(e) = run_grpc_server(GRPC_PORT).await {
            eprintln!("gRPC server error: {}", e);
        }
    });
    
    // Wait for gRPC server to start
    sleep(Duration::from_millis(500)).await;
    
    // Create gRPC client
    let mut client = ndn_service::ndn_router_client::NdnRouterClient::connect(
        format!("http://127.0.0.1:{}", GRPC_PORT)
    ).await?;
    
    // Send Interest through gRPC which will be forwarded over QUIC
    let interest_request = ndn_service::InterestRequest {
        name: "/test/data".to_string(),
        remote_server: format!("127.0.0.1:{}", QUIC_SERVER_PORT),
        can_be_prefix: false,
        must_be_fresh: true,
        lifetime_ms: 5000,
    };
    
    println!("Sending Interest via gRPC");
    let response = client.send_interest(Request::new(interest_request)).await?;
    let data_response = response.into_inner();
    
    // Validate the response
    println!("Received Data response via gRPC: {}", data_response.name);
    assert!(data_response.success, "Data response was not successful");
    assert_eq!(data_response.name, "/test/data", "Incorrect name in Data response");
    
    // Convert the content to a string and check it
    let content_str = String::from_utf8_lossy(&data_response.content);
    println!("Data content: {}", content_str);
    assert_eq!(
        content_str,
        "This is test data for /test/data",
        "Incorrect content in Data response"
    );
    
    // Clean up
    quic_server_handle.abort();
    grpc_server_handle.abort();
    
    println!("Test completed successfully");
    Ok(())
}
