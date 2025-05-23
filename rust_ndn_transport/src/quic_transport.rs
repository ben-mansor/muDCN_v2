// μDCN QUIC Transport Implementation
//
// This module implements a simplified QUIC transport layer for NDN
// using the quinn crate. It provides a clean interface for exchanging
// Interest and Data packets over QUIC streams.

use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut, BufMut};
use dashmap::DashMap;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig, TransportConfig};
use rustls::{Certificate, PrivateKey, client::ServerCertVerifier, Error as RustlsError};
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn, trace};

/// An insecure certificate verifier that accepts any server certificate
/// WARNING: This should only be used for development and testing
struct InsecureServerVerifier {}

impl ServerCertVerifier for InsecureServerVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, RustlsError> {
        // WARNING: This accepts any certificate without verification
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

use crate::error::{Error, Result};
use crate::ndn::{Data, Interest};
use crate::name::Name;
use crate::security::generate_self_signed_cert;

/// Connection state tracking enum
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,
    /// Connection is established and active
    Connected,
    /// Connection is idle (no recent activity)
    Idle,
    /// Connection is closing or has closed
    Closing,
    /// Connection has failed with reason
    Failed(String),
}

/// Connection statistics for monitoring and diagnostics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Round-trip time in milliseconds (moving average)
    pub rtt_ms: u64,
    /// Number of interests sent
    pub interests_sent: u64,
    /// Number of data packets received
    pub data_received: u64,
    /// Number of timeouts encountered
    pub timeouts: u64,
    /// Number of errors encountered
    pub errors: u64,
    /// Last activity timestamp
    pub last_activity: Instant,
    /// Average data packet size
    pub avg_data_size: usize,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            rtt_ms: 0,
            interests_sent: 0,
            data_received: 0,
            timeouts: 0,
            errors: 0,
            last_activity: Instant::now(),
            avg_data_size: 0,
        }
    }
}

/// A thread-safe tracker for QUIC connections that maintains state and statistics
#[derive(Debug)]
pub struct ConnectionTracker {
    /// The underlying QUIC connection
    connection: Connection,
    /// Current connection state
    state: RwLock<ConnectionState>,
    /// Connection statistics
    stats: RwLock<ConnectionStats>,
    /// Remote peer address
    remote_addr: SocketAddr,
}

impl ConnectionTracker {
    /// Create a new connection tracker
    pub fn new(connection: Connection, remote_addr: SocketAddr) -> Self {
        Self {
            connection,
            state: RwLock::new(ConnectionState::Connecting),
            stats: RwLock::new(ConnectionStats::default()),
            remote_addr,
        }
    }

    /// Update connection state
    pub async fn set_state(&self, state: ConnectionState) {
        let mut current_state = self.state.write().await;
        *current_state = state;
    }

    /// Get the current connection state
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Report successful interest/data exchange
    pub async fn report_success(&self, rtt_ms: u64, data_size: usize) {
        let mut stats = self.stats.write().await;
        
        // Update RTT (moving average with 20% weight for new value)
        if stats.rtt_ms == 0 {
            stats.rtt_ms = rtt_ms;
        } else {
            stats.rtt_ms = (stats.rtt_ms * 8 + rtt_ms * 2) / 10;
        }
        
        // Update interest sent and data received counters
        stats.interests_sent += 1;
        stats.data_received += 1;
        
        // Update last activity timestamp
        stats.last_activity = Instant::now();
        
        // Update average data size (moving average)
        if stats.avg_data_size == 0 {
            stats.avg_data_size = data_size;
        } else {
            stats.avg_data_size = (stats.avg_data_size * 8 + data_size * 2) / 10;
        }

        debug!("Connection stats updated: RTT={}ms, interests={}, data={}", 
               stats.rtt_ms, stats.interests_sent, stats.data_received);
    }

    /// Report a failure (timeout or error)
    pub async fn report_failure(&self, is_timeout: bool, reason: &str) {
        let mut stats = self.stats.write().await;
        
        if is_timeout {
            stats.timeouts += 1;
            warn!("Connection timeout: {}", reason);
        } else {
            stats.errors += 1;
            error!("Connection error: {}", reason);
        }
        
        // Still count as an interest sent
        stats.interests_sent += 1;
        
        // Update last activity timestamp
        stats.last_activity = Instant::now();
    }

    /// Get connection statistics
    pub async fn stats(&self) -> ConnectionStats {
        self.stats.read().await.clone()
    }

    /// Check if connection is idle
    pub async fn is_idle(&self, idle_threshold: Duration) -> bool {
        let stats = self.stats.read().await;
        let idle = stats.last_activity.elapsed() > idle_threshold;
        if idle {
            debug!("Connection to {} idle for {:?}", self.remote_addr, stats.last_activity.elapsed());
        }
        idle
    }

    /// Get the underlying connection
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
    
    /// Get the remote peer address
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
}

/// Handler type for Interest packets - processes an Interest and returns a Data packet
pub type InterestHandler = Arc<dyn Fn(Interest) -> Result<Data> + Send + Sync>;

/// The main QUIC transport implementation for NDN
#[derive(Debug)]
pub struct QuicTransport {
    /// QUIC endpoint that manages connections
    endpoint: Endpoint,
    /// Active connections tracked by remote address
    connections: DashMap<SocketAddr, Arc<ConnectionTracker>>,
    /// Handle for the server task
    server_handle: Option<JoinHandle<()>>,
    /// Registered handlers for Interest packets based on name prefix
    handlers: Arc<RwLock<HashMap<Name, InterestHandler>>>,
    /// Idle timeout in seconds
    idle_timeout: u64,
    /// Maximum packet size
    max_packet_size: usize,
    /// Server status tracking
    server_running: Arc<Mutex<bool>>,
    /// Local bind address
    bind_addr: SocketAddr,
    /// Local port
    port: u16,
}

impl QuicTransport {
    /// Create a new QUIC transport instance
    pub async fn new(
        bind_addr: &str, 
        port: u16, 
        idle_timeout_secs: u64, 
        max_packet_size: usize
    ) -> Result<Self> {
        // Parse bind address
        let addr = format!("{}:{}", bind_addr, port).parse::<SocketAddr>()?;
        
        // Generate self-signed certificate
        let (cert, key) = generate_self_signed_cert()?;
        
        // Create server config
        let server_config = create_server_config(vec![cert], key)?;
        
        // Create endpoint
        let endpoint = Endpoint::server(server_config, addr)?;
        info!("QUIC endpoint bound to {}", addr);
        
        Ok(Self {
            endpoint,
            connections: DashMap::new(),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            server_handle: None,
            idle_timeout: idle_timeout_secs,
            max_packet_size,
            server_running: Arc::new(Mutex::new(false)),
            bind_addr: addr,
            port,
        })
    }
    
    /// Helper method to create a proper transport configuration
    fn create_transport_config(idle_timeout: u64) -> Result<TransportConfig> {
        let mut transport_config = TransportConfig::default();
        
        // Set keepalive interval (15 seconds)
        transport_config.keep_alive_interval(Some(Duration::from_secs(15)));
        
        // Set idle timeout
        transport_config.max_idle_timeout(Some(Duration::from_secs(idle_timeout).try_into().unwrap()));
        
        // Allow a reasonably large number of concurrent bi-directional streams
        transport_config.max_concurrent_bidi_streams(100_u32.into());
        
        // Set receive window (8MB)
        transport_config.receive_window(8_000_000u32);
        
        // Set send window (8MB)
        transport_config.send_window(8_000_000u32);
        
        // Set initial MTU (1400 is a safe default)
        transport_config.initial_mtu(1400);
        
        Ok(transport_config)
    }
    
    /// Create server configuration with the provided certificate and key
    fn create_server_config(certs: Vec<Certificate>, key: PrivateKey) -> Result<ServerConfig> {
        let mut server_config = ServerConfig::with_single_cert(certs, key)
            .map_err(|e| Error::CryptoError(format!("Failed to create server config: {}", e)))?;
        
        // Configure transport parameters
        let transport_config = Self::create_transport_config(30)?; // 30 second default idle timeout for server
        
        // Apply transport config to server config
        server_config.transport_config(transport_config);
        
        Ok(server_config)
    }
    
    /// Create client configuration for connecting to servers
    fn create_client_config(idle_timeout: u64) -> Result<ClientConfig> {
        // Create a basic client config without certificate verification for development
        let crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(InsecureServerVerifier {}))
            .with_no_client_auth();
            
        let mut client_config = ClientConfig::new(Arc::new(crypto));
        
        // Apply transport configuration
        let transport_config = Self::create_transport_config(idle_timeout)?;
        client_config.transport_config(transport_config);
        
        Ok(client_config)
    }
    
    /// Start the QUIC transport server
    pub async fn start_server(&mut self) -> Result<()> {
        // Set the server as running
        let mut running = self.server_running.lock().await;
        if *running {
            return Err(Error::AlreadyRunning("Server already running".to_string()));
        }
        *running = true;
        drop(running);
        
        // Clone required references for the server task
        let endpoint = self.endpoint.clone();
        let handlers = self.handlers.clone();
        let connections = self.connections.clone();
        let max_packet_size = self.max_packet_size;
        let server_running = self.server_running.clone();
        
        // Start the server task
        self.server_handle = Some(tokio::spawn(async move {
            info!("QUIC transport server started, accepting connections");
            
            // Loop while accepting connections
            while let Some(conn) = endpoint.accept().await {
                info!("Incoming connection from {:?}", conn.remote_address());
                let remote = conn.remote_address();
                
                // Accept the connection
                match conn.await {
                    Ok(connection) => {
                        info!("Connection established with {}", remote);
                        
                        // Create connection tracker
                        let conn_tracker = Arc::new(ConnectionTracker::new(connection, remote));
                        conn_tracker.set_state(ConnectionState::Connected).await;
                        
                        // Add to known connections
                        connections.insert(remote, conn_tracker.clone());
                        
                        // Clone required handlers for this connection
                        let handlers = handlers.clone();
                        let connections = connections.clone();
                        
                        // Handle this connection in separate task
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_connection(conn_tracker.clone(), handlers, max_packet_size).await {
                                error!("Connection error: {}", e);
                                conn_tracker.set_state(ConnectionState::Failed(e.to_string())).await;
                            }
                            
                            // Remove connection when done
                            connections.remove(&remote);
                        });
                    },
                    Err(e) => {
                        error!("Connection failed: {}", e);
                    }
                }
            }
            
            // Server loop exited
            info!("QUIC transport server stopped");
            if let Ok(mut running) = server_running.lock().await {
                *running = false;
            }
        }));
        
        Ok(())
    }
    
    /// Handle a QUIC connection
    async fn handle_connection(
        conn_tracker: Arc<ConnectionTracker>,
        handlers: Arc<RwLock<HashMap<Name, InterestHandler>>>,
        max_packet_size: usize,
    ) -> Result<()> {
        let connection = conn_tracker.connection().clone();
        let remote_addr = conn_tracker.remote_addr();
        
        info!("Handling new connection from {}", remote_addr);
        
        // Accept bi-directional streams initiated by the peer
        while let Ok((send, recv)) = connection.accept_bi().await {
            // Clone handlers for this stream
            let handlers = handlers.clone();
            let conn_tracker = conn_tracker.clone();
            
            // Handle stream in a new task
            tokio::spawn(async move {
                if let Err(e) = Self::handle_stream(send, recv, handlers, conn_tracker.clone(), max_packet_size).await {
                    error!("Stream handling error: {}", e);
                }
            });
        }
        
        // Connection closed
        conn_tracker.set_state(ConnectionState::Closing).await;
        info!("Connection from {} closed", remote_addr);
        
        Ok(())
    }
    
    /// Handle a bi-directional QUIC stream
    async fn handle_stream(
        mut send: SendStream,
        mut recv: RecvStream,
        handlers: Arc<RwLock<HashMap<Name, InterestHandler>>>,
        conn_tracker: Arc<ConnectionTracker>,
        max_packet_size: usize,
    ) -> Result<()> {
        // Read the stream until we have the Interest packet
        let interest_bytes = match recv.read_to_end(max_packet_size).await {
            Ok(bytes) => bytes,
            Err(e) => {
                conn_tracker.report_failure(false, &format!("Stream read error: {}", e)).await;
                return Err(Error::IoError(format!("Failed to read from stream: {}", e)))
            }
        };
        
        // Parse the Interest packet
        let interest = match Interest::from_bytes(&interest_bytes) {
            Ok(interest) => {
                debug!("Received Interest for {}", interest.name());
                interest
            },
            Err(e) => {
                conn_tracker.report_failure(false, &format!("Interest parsing error: {}", e)).await;
                return Err(Error::ParsingError(format!("Failed to parse Interest: {}", e)))
            }
        };
        
        // Find a handler for this Interest
        let handlers = handlers.read().await;
        let mut matching_handler = None;
        let mut longest_prefix = 0;
        
        // Find the best matching prefix (longest match)
        for (prefix, handler) in handlers.iter() {
            if interest.name().starts_with(prefix) && prefix.len() > longest_prefix {
                matching_handler = Some(handler);
                longest_prefix = prefix.len();
            }
        }
        
        // Process the Interest with the matching handler
        if let Some(handler) = matching_handler {
            // Get start time for RTT calculation
            let start_time = Instant::now();
            
            // Call handler to get Data response
            match handler(interest.clone()) {
                Ok(data) => {
                    // Encode Data packet
                    let data_bytes = data.to_bytes();
                    
                    // Send Data response
                    if let Err(e) = send.write_all(&data_bytes).await {
                        conn_tracker.report_failure(false, &format!("Write error: {}", e)).await;
                        return Err(Error::IoError(format!("Failed to send Data: {}", e)))
                    }
                    
                    // Finish the stream
                    if let Err(e) = send.finish().await {
                        warn!("Error finishing stream: {}", e);
                    }
                    
                    // Calculate RTT
                    let rtt = start_time.elapsed().as_millis() as u64;
                    let data_size = data_bytes.len();
                    
                    // Update connection statistics
                    conn_tracker.report_success(rtt, data_size).await;
                    
                    debug!("Sent Data response for {}, size={} bytes, RTT={}ms", 
                          interest.name(), data_size, rtt);
                },
                Err(e) => {
                    conn_tracker.report_failure(false, &format!("Handler error: {}", e)).await;
                    return Err(e);
                }
            }
        } else {
            // No handler found
            conn_tracker.report_failure(false, &format!("No handler for {}", interest.name())).await;
            return Err(Error::Other(format!("No handler for {}", interest.name())));
        }
        
        Ok(())
    }
    
    /// Register a handler for a specific name prefix
    pub async fn register_handler(
        &self,
        prefix: Name,
        handler: impl Fn(Interest) -> Result<Data> + Send + Sync + 'static
    ) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.insert(prefix.clone(), Arc::new(handler));
        info!("Registered handler for prefix: {}", prefix);
        Ok(())
    }
    
    /// Connect to a remote QUIC NDN server
    pub async fn connect(&self, remote_addr: &str, remote_port: u16) -> Result<Arc<ConnectionTracker>> {
        // Parse remote address
        let addr = format!("{}:{}", remote_addr, remote_port).parse::<SocketAddr>()
            .map_err(|e| Error::AddrParseError(format!("Failed to parse address: {}", e)))?;
        
        // Check if we already have a connection to this address
        if let Some(conn) = self.connections.get(&addr) {
            return Ok(conn.clone());
        }
        
        // Create client config
        let client_config = Self::create_client_config(self.idle_timeout)?;
        
        // Connect to the remote endpoint
        info!("Connecting to {}...", addr);
        let connecting = self.endpoint.connect_with(client_config, addr, "localhost")
            .map_err(|e| Error::ConnectionError(format!("Failed to connect: {}", e)))?;
        
        // Wait for connection to be established
        let connection = connecting.await
            .map_err(|e| Error::ConnectionError(format!("Connection failed: {}", e)))?;
        
        info!("Connected to {}", addr);
        
        // Create connection tracker
        let conn_tracker = Arc::new(ConnectionTracker::new(connection, addr));
        conn_tracker.set_state(ConnectionState::Connected).await;
        
        // Store connection
        self.connections.insert(addr, conn_tracker.clone());
        
        Ok(conn_tracker)
    }
    
    /// Send an Interest packet to a remote peer and wait for Data
    pub async fn send_interest(&self, remote_addr: SocketAddr, interest: Interest) -> Result<Data> {
        // Get connection to the remote peer (or error if not connected)
        let conn_tracker = match self.connections.get(&remote_addr) {
            Some(tracker) => tracker.clone(),
            None => return Err(Error::ConnectionError(format!("No connection to {}", remote_addr)))
        };
        
        // Get the connection
        let connection = conn_tracker.connection().clone();
        
        // Measure start time for RTT calculation
        let start_time = Instant::now();
        
        // Open a bi-directional stream
        let (mut send, mut recv) = connection.open_bi().await
            .map_err(|e| Error::ConnectionError(format!("Failed to open stream: {}", e)))?;
        
        // Encode Interest
        let interest_bytes = interest.to_bytes();
        debug!("Sending Interest for {}, size={} bytes", interest.name(), interest_bytes.len());
        
        // Send Interest
        send.write_all(&interest_bytes).await
            .map_err(|e| Error::IoError(format!("Failed to send Interest: {}", e)))?;
        
        // Finish sending
        send.finish().await
            .map_err(|e| Error::IoError(format!("Failed to finish stream: {}", e)))?;
        
        // Wait for Data
        match recv.read_to_end(self.max_packet_size).await {
            Ok(data_bytes) => {
                // Calculate RTT
                let rtt = start_time.elapsed().as_millis() as u64;
                
                // Decode Data
                match Data::from_bytes(&data_bytes) {
                    Ok(data) => {
                        // Update statistics
                        conn_tracker.report_success(rtt, data_bytes.len()).await;
                        
                        debug!("Received Data for {}, size={} bytes, RTT={}ms", 
                               interest.name(), data_bytes.len(), rtt);
                        
                        Ok(data)
                    },
                    Err(e) => {
                        conn_tracker.report_failure(false, &format!("Data parsing error: {}", e)).await;
                        Err(Error::ParsingError(format!("Failed to decode Data: {}", e)))
                    }
                }
            },
            Err(e) => {
                // Handle timeout or other errors
                let is_timeout = e.to_string().contains("timeout");
                conn_tracker.report_failure(is_timeout, &format!("Receive error: {}", e)).await;
                
                if is_timeout {
                    Err(Error::Timeout(format!("Interest timed out: {}", interest.name())))
                } else {
                    Err(Error::IoError(format!("Failed to receive Data: {}", e)))
                }
            }
        }
    }
    
    /// Close a specific connection
    pub async fn close_connection(&self, remote_addr: SocketAddr) -> Result<()> {
        if let Some(conn_tracker) = self.connections.get(&remote_addr) {
            info!("Closing connection to {}", remote_addr);
            
            // Update state
            conn_tracker.set_state(ConnectionState::Closing).await;
            
            // Close connection
            conn_tracker.connection().close(0u32.into(), b"connection closed by application");
            
            // Remove from map
            self.connections.remove(&remote_addr);
            
            Ok(())
        } else {
            Err(Error::ConnectionError(format!("No connection to {}", remote_addr)))
        }
    }
    
    /// Shutdown the transport server and all connections
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down QUIC transport");
        
        // Stop server task
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
        
        // Close all connections
        for conn in self.connections.iter() {
            let addr = *conn.key();
            info!("Closing connection to {}", addr);
            conn.value().connection().close(0u32.into(), b"server shutting down");
        }
        
        // Clear connections map
        self.connections.clear();
        
        // Close endpoint
        self.endpoint.close(0u32.into(), b"server shutting down");
        
        // Update server status
        let mut running = self.server_running.lock().await;
        *running = false;
        
        info!("QUIC transport shutdown complete");
        Ok(())
    }
    
    /// Get statistics for a specific connection
    pub async fn get_connection_stats(&self, remote_addr: SocketAddr) -> Option<ConnectionStats> {
        if let Some(conn_tracker) = self.connections.get(&remote_addr) {
            Some(conn_tracker.stats().await)
        } else {
            None
        }
    }
    
    /// Get a list of all active connection addresses
    pub fn get_connections(&self) -> Vec<SocketAddr> {
        self.connections.iter().map(|entry| *entry.key()).collect()
    }
    
    // Handle a QUIC connection
    async fn handle_connection(
        conn: Connection,
        remote: SocketAddr,
        handlers: Arc<RwLock<HashMap<Name, InterestHandler>>>,
        conn_tracker: Arc<ConnectionTracker>,
        max_packet_size: usize
    ) {
        info!("Handling connection from {}", remote);
        
        // Process incoming streams
        while let Ok((mut send, mut recv)) = conn.accept_bi().await {
            // Handle the stream in a separate task
            let handlers_clone = handlers.clone();
            let conn_tracker_clone = conn_tracker.clone();
            let max_packet_size_clone = max_packet_size;
            
            tokio::spawn(async move {
                Self::handle_stream(
                    &mut send,
                    &mut recv,
                    handlers_clone,
                    conn_tracker_clone,
                    max_packet_size_clone
                ).await;
            });
        }
        
        info!("Connection handler finished for {}", remote);
        conn_tracker.set_state(ConnectionState::Closing).await;
    }
    
    // Handle a QUIC stream
    async fn handle_stream(
        send: &mut SendStream,
        recv: &mut RecvStream,
        handlers: Arc<RwLock<HashMap<Name, InterestHandler>>>,
        conn_tracker: Arc<ConnectionTracker>,
        max_packet_size: usize
    ) {
        // Read the Interest packet
        let interest_bytes = match recv.read_to_end(max_packet_size).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Error reading from stream: {}", e);
                conn_tracker.report_failure(false).await;
                return;
            }
        };
        
        // Decode Interest
        let interest = match Interest::from_bytes(&interest_bytes) {
            Ok(interest) => interest,
            Err(e) => {
                error!("Error decoding Interest: {}", e);
                conn_tracker.report_failure(false).await;
                return;
            }
        };
        
        debug!("Received Interest for {}", interest.name());
        
        // Find handler for this name
        let handlers_guard = handlers.read().await;
        let mut handler_opt = None;
        let mut longest_prefix = 0;
        
        for (prefix, handler) in handlers_guard.iter() {
            if interest.name().has_prefix(prefix) && prefix.len() > longest_prefix {
                handler_opt = Some(handler.clone());
                longest_prefix = prefix.len();
            }
        }
        
        // Process Interest
        let response = match handler_opt {
            Some(handler) => {
                match handler(interest) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Handler error: {}", e);
                        conn_tracker.report_failure(false).await;
                        return;
                    }
                }
            },
            None => {
                // No handler found, create a simple NACK response
                warn!("No handler for {}", interest.name());
                conn_tracker.report_failure(false).await;
                return;
            }
        };
        
        // Encode Data
        let data_bytes = response.to_bytes();
        
        // Send Data
        match send.write_all(&data_bytes).await {
            Ok(_) => {
                debug!("Sent Data for {}", interest.name());
                conn_tracker.report_success(0, data_bytes.len()).await;
            },
            Err(e) => {
                error!("Error sending Data: {}", e);
                conn_tracker.report_failure(false).await;
            }
        }
        
        // Finish sending
        if let Err(e) = send.finish().await {
            error!("Error finishing stream: {}", e);
        }
    }
    
    // Register a handler for a name prefix
    pub async fn register_handler(&self, prefix: Name, handler: impl Fn(Interest) -> Result<Data> + Send + Sync + 'static) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.insert(prefix.clone(), Arc::new(handler));
        info!("Registered handler for prefix: {}", prefix);
        Ok(())
    }
    
    // Connect to a remote NDN node
    pub async fn connect(&self, remote_addr: &str, remote_port: u16) -> Result<Arc<ConnectionTracker>> {
        // Parse remote address
        let addr = format!("{}:{}", remote_addr, remote_port).parse::<SocketAddr>()?;
        
        // Check if we already have a connection
        if let Some(conn) = self.connections.get(&addr) {
            return Ok(conn.clone());
        }
        
        // Create client config
        let client_config = create_client_config()?;
        
        // Connect to the remote endpoint
        info!("Connecting to {}:{}", remote_addr, remote_port);
        let connecting = self.endpoint.connect_with(client_config, addr, "localhost")?;
        
        // Wait for connection
        let connection = connecting.await?;
        
        // Create connection tracker
        let conn_tracker = Arc::new(ConnectionTracker::new(connection));
        conn_tracker.set_state(ConnectionState::Connected).await;
        
        // Store the connection
        self.connections.insert(addr, conn_tracker.clone());
        
        Ok(conn_tracker)
    }
    
    // Send an Interest packet and wait for Data
    pub async fn send_interest(&self, remote_addr: SocketAddr, interest: Interest) -> Result<Data> {
        // Get or create connection
        let conn_tracker = if let Some(tracker) = self.connections.get(&remote_addr) {
            tracker.clone()
        } else {
            // We need to connect first - but this should normally be done explicitly
            return Err(Error::ConnectionError("Not connected to remote peer".to_string()));
        };
        
        // Check connection state
        let state = conn_tracker.state().await;
        if state != ConnectionState::Connected {
            return Err(Error::ConnectionError(format!("Connection not ready: {:?}", state)));
        }
        
        // Start time for RTT measurement
        let start_time = Instant::now();
        
        // Open bidirectional stream
        let connection = conn_tracker.connection();
        let (mut send, mut recv) = connection.open_bi().await
            .map_err(|e| Error::ConnectionError(format!("Failed to open stream: {}", e)))?;
        
        // Encode Interest
        let interest_bytes = interest.to_bytes();
        
        // Send Interest
        send.write_all(&interest_bytes).await
            .map_err(|e| Error::IoError(format!("Failed to send Interest: {}", e)))?;
        
        // Finish sending
        send.finish().await
            .map_err(|e| Error::IoError(format!("Failed to finish stream: {}", e)))?;
        
        debug!("Sent Interest for {}", interest.name());
        
        // Wait for Data
        let data_bytes = recv.read_to_end(self.max_packet_size).await
            .map_err(|e| Error::IoError(format!("Failed to receive Data: {}", e)))?;
        
        // Calculate RTT
        let rtt = start_time.elapsed().as_millis() as u64;
        
        // Decode Data
        let data = Data::from_bytes(&data_bytes)
            .map_err(|e| Error::ParsingError(format!("Failed to decode Data: {}", e)))?;
        
        // Update statistics
        conn_tracker.report_success(rtt, data_bytes.len()).await;
        
        debug!("Received Data for {}, RTT: {}ms", interest.name(), rtt);
        
        Ok(data)
    }
    
    // Close a connection
    pub async fn close_connection(&self, remote_addr: SocketAddr) -> Result<()> {
        if let Some(conn_tracker) = self.connections.get(&remote_addr) {
            conn_tracker.set_state(ConnectionState::Closing).await;
            let connection = conn_tracker.connection();
            connection.close(0u32.into(), b"connection closed by application");
            self.connections.remove(&remote_addr);
            Ok(())
        } else {
            Err(Error::ConnectionError("Connection not found".to_string()))
        }
    }
    
    // Shutdown the transport
    pub async fn shutdown(&mut self) -> Result<()> {
        // Stop server task
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
        }
        
        // Close all connections
        for conn in self.connections.iter() {
            let connection = conn.connection();
            connection.close(0u32.into(), b"server shutting down");
        }
        
        self.connections.clear();
        self.endpoint.close(0u32.into(), b"server shutting down");
        
        Ok(())
    }
    
    // Get connection statistics for a remote address
    pub async fn get_connection_stats(&self, remote_addr: SocketAddr) -> Option<ConnectionStats> {
        if let Some(conn_tracker) = self.connections.get(&remote_addr) {
            Some(conn_tracker.stats().await)
        } else {
            None
        }
    }
    
    // Get all active connections
    pub fn get_connections(&self) -> Vec<SocketAddr> {
        self.connections.iter().map(|entry| *entry.key()).collect()
    }
}

// Helper function to create a server configuration
fn create_server_config(certs: Vec<Certificate>, key: PrivateKey) -> Result<ServerConfig> {
    let mut server_config = ServerConfig::with_single_cert(certs, key)
        .map_err(|e| Error::CryptoError(format!("Failed to create server config: {}", e)))?;
    
    // Configure transport parameters
    let transport_config = Arc::get_mut(&mut server_config.transport)
        .ok_or_else(|| Error::Other("Failed to get transport config".to_string()))?;
    
    // Set keepalive interval
    transport_config.keep_alive_interval(Some(Duration::from_secs(15)));
    
    // Set idle timeout
    transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
    
    Ok(server_config)
}

// Helper function to create a client configuration
fn create_client_config() -> Result<ClientConfig> {
    // Use basic client config without certificate verification for development
    let client_config = ClientConfig::new(Arc::new(rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth()
    ));
    
    Ok(client_config)
}
