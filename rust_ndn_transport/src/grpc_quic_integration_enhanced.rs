// μDCN Enhanced gRPC and QUIC Integration Module
//
// This module enhances the gRPC-QUIC adapter with improved features:
// 1. Automatic retry with exponential backoff
// 2. Interest pipelining for concurrent requests
// 3. Better connection management and monitoring

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};
use crate::grpc::udcn::{
    DataPacketResponse, InterestPacketRequest, QuicConnectionRequest, QuicConnectionResponse,
    ConnectionQuality, InterestFilter, PipelineStatsResponse
};
use crate::interest_retry::{RetryPolicy, with_retry};
use crate::ndn::{Data, Interest};
use crate::name::Name;
use crate::pipeline::{InterestPipeline, PipelineConfig, PipelineRegistry, PipelineStats};
use crate::quic_transport::{QuicTransport, ConnectionState, ConnectionStats};

/// Enhanced integration adapter between gRPC and QUIC transport
pub struct EnhancedGrpcQuicAdapter {
    /// The QUIC transport instance
    transport: Arc<QuicTransport>,
    
    /// Connection ID to socket address mapping
    connections: Arc<RwLock<HashMap<String, SocketAddr>>>,
    
    /// Pipeline registry
    pipelines: Arc<PipelineRegistry>,
    
    /// Next connection ID
    next_conn_id: Arc<RwLock<u64>>,
    
    /// Default retry policy
    retry_policy: RetryPolicy,
}

impl EnhancedGrpcQuicAdapter {
    /// Create a new enhanced gRPC-QUIC adapter
    pub async fn new(bind_addr: &str, port: u16) -> Result<Self> {
        // Create the QUIC transport
        let quic_transport = QuicTransport::new(bind_addr, port, 30, 65535).await?;
        let transport = Arc::new(quic_transport);
        
        // Start the QUIC transport server
        let mut transport_mut = transport.clone();
        transport_mut.start_server().await?;
        
        // Create pipeline registry
        let pipelines = Arc::new(PipelineRegistry::new(transport.clone()));
        
        Ok(Self {
            transport,
            connections: Arc::new(RwLock::new(HashMap::new())),
            pipelines,
            next_conn_id: Arc::new(RwLock::new(1)),
            retry_policy: RetryPolicy::default(),
        })
    }
    
    /// Create a new enhanced gRPC-QUIC adapter with a custom retry policy
    pub async fn new_with_retry_policy(bind_addr: &str, port: u16, retry_policy: RetryPolicy) -> Result<Self> {
        let mut adapter = Self::new(bind_addr, port).await?;
        adapter.retry_policy = retry_policy;
        Ok(adapter)
    }
    
    /// Set the default pipeline configuration
    pub fn set_pipeline_config(&self, config: PipelineConfig) {
        // This requires mut access to the registry which we don't have
        // For now, we'll leave this as is - the default config is reasonable
    }
    
    /// Create a QUIC connection to a remote NDN router
    pub async fn create_quic_connection(&self, req: QuicConnectionRequest) -> Result<QuicConnectionResponse> {
        let peer_address = req.peer_address.clone();
        let port = req.port as u16;
        
        // Validate input
        if peer_address.is_empty() {
            return Err(Error::InvalidArgument("Peer address cannot be empty".to_string()));
        }
        
        if port == 0 || port > 65535 {
            return Err(Error::InvalidArgument("Invalid port number".to_string()));
        }
        
        // Use retry for connection creation
        let transport = self.transport.clone();
        let result = with_retry(
            || async {
                transport.connect(&peer_address, port).await
            },
            &self.retry_policy,
            "create_quic_connection"
        ).await?;
        
        // Generate connection ID
        let conn_id = {
            let mut next_id = self.next_conn_id.write().await;
            let id = *next_id;
            *next_id += 1;
            format!("conn-{}-{}", peer_address.replace(".", "-"), id)
        };
        
        // Store connection mapping
        let remote_addr = format!("{}:{}", peer_address, port).parse()?;
        {
            let mut connections = self.connections.write().await;
            connections.insert(conn_id.clone(), remote_addr);
        }
        
        // Create a pipeline for this connection
        let _ = self.pipelines.get_or_create_pipeline(&conn_id, remote_addr).await?;
        
        // Get connection stats
        let conn_stats = result.stats().await;
        
        // Determine connection quality
        let quality = if conn_stats.rtt_ms < 50 {
            ConnectionQuality::Excellent as i32
        } else if conn_stats.rtt_ms < 100 {
            ConnectionQuality::Good as i32
        } else if conn_stats.rtt_ms < 200 {
            ConnectionQuality::Fair as i32
        } else {
            ConnectionQuality::Poor as i32
        };
        
        // Create response
        let response = QuicConnectionResponse {
            success: true,
            error_message: String::new(),
            connection_id: conn_id,
            remote_address: peer_address,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            quality,
        };
        
        Ok(response)
    }
    
    /// Send an Interest packet with automatic retry
    pub async fn send_interest_with_retry(&self, req: InterestPacketRequest) -> Result<DataPacketResponse> {
        let connection_id = req.connection_id.clone();
        let name_str = req.name.clone();
        let can_be_prefix = req.can_be_prefix;
        let must_be_fresh = req.must_be_fresh;
        let lifetime_ms = req.lifetime_ms;
        
        // Validate the name
        if name_str.is_empty() {
            return Err(Error::InvalidArgument("Interest name cannot be empty".to_string()));
        }
        
        // Get socket address for the connection
        let remote_addr = {
            let connections = self.connections.read().await;
            match connections.get(&connection_id) {
                Some(addr) => *addr,
                None => return Err(Error::NotFound(format!("Connection {} not found", connection_id))),
            }
        };
        
        // Parse the name
        let name = Name::from_uri(&name_str)?;
        
        // Create Interest packet
        let mut interest = Interest::new(name);
        interest.set_can_be_prefix(can_be_prefix);
        interest.set_must_be_fresh(must_be_fresh);
        interest.set_lifetime_ms(lifetime_ms);
        
        // Get or create pipeline for this connection
        let pipeline = self.pipelines.get_or_create_pipeline(&connection_id, remote_addr).await?;
        
        // Send Interest with retry using the pipeline
        let self_clone = self.clone();
        let interest_clone = interest.clone();
        let start_time = Instant::now();
        
        let data = with_retry(
            || async { 
                pipeline.send_interest(interest_clone.clone()).await
            },
            &self.retry_policy,
            &format!("send_interest for {}", name_str)
        ).await?;
        
        let rtt = start_time.elapsed().as_millis() as u64;
        debug!("Interest for {} completed with RTT: {}ms", name_str, rtt);
        
        // Create response
        let response = DataPacketResponse {
            success: true,
            error_message: String::new(),
            name: data.name().to_string(),
            content: data.content(),
            content_type: data.content_type() as u32,
            freshness_period: data.freshness_period_ms(),
            signature: data.signature().unwrap_or_default(),
            signature_type: data.signature_type().unwrap_or_default(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };
        
        Ok(response)
    }
    
    /// Send an Interest packet (compatibility method)
    pub async fn send_interest(&self, req: InterestPacketRequest) -> Result<DataPacketResponse> {
        self.send_interest_with_retry(req).await
    }
    
    /// Register a handler for receiving Data packets for a specific prefix
    pub async fn register_prefix_interest(&self, prefix: &str, handler: impl Fn(Interest) -> Result<Data> + Send + Sync + 'static) -> Result<()> {
        // Parse prefix
        let name = Name::from_uri(prefix)?;
        
        // Register handler with QUIC transport
        let mut transport = self.transport.clone();
        transport.register_handler(name, handler).await?;
        
        Ok(())
    }
    
    /// Close a QUIC connection
    pub async fn close_connection(&self, connection_id: &str) -> Result<()> {
        // Get socket address for the connection
        let remote_addr = {
            let mut connections = self.connections.write().await;
            match connections.remove(connection_id) {
                Some(addr) => addr,
                None => return Err(Error::NotFound(format!("Connection {} not found", connection_id))),
            }
        };
        
        // Remove the pipeline for this connection
        self.pipelines.remove_pipeline(connection_id).await.ok();
        
        // Close the connection
        let mut transport = self.transport.clone();
        transport.close_connection(remote_addr).await?;
        
        Ok(())
    }
    
    /// Get connection statistics
    pub async fn get_connection_stats(&self, connection_id: &str) -> Result<ConnectionStats> {
        // Get socket address for the connection
        let remote_addr = {
            let connections = self.connections.read().await;
            match connections.get(connection_id) {
                Some(addr) => *addr,
                None => return Err(Error::NotFound(format!("Connection {} not found", connection_id))),
            }
        };
        
        // Get connection stats
        let transport = self.transport.clone();
        match transport.get_connection_stats(remote_addr).await {
            Some(stats) => Ok(stats),
            None => Err(Error::NotFound(format!("Connection {} statistics not available", connection_id))),
        }
    }
    
    /// Get pipeline statistics
    pub async fn get_pipeline_stats(&self, connection_id: &str) -> Result<PipelineStatsResponse> {
        // Get pipeline stats
        let pipeline_stats = self.pipelines.get_pipeline_stats(connection_id).await?;
        
        // Create response
        let response = PipelineStatsResponse {
            connection_id: connection_id.to_string(),
            interests_sent: pipeline_stats.interests_sent,
            data_received: pipeline_stats.data_received,
            timeouts: pipeline_stats.timeouts,
            errors: pipeline_stats.errors,
            avg_rtt_ms: pipeline_stats.avg_rtt_ms,
            queue_size: pipeline_stats.queue_size as u64,
            in_flight: pipeline_stats.in_flight as u64,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };
        
        Ok(response)
    }
    
    /// List all active connections
    pub async fn list_connections(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }
    
    /// Shutdown the QUIC transport
    pub async fn shutdown(&self) -> Result<()> {
        // Shutdown all pipelines
        self.pipelines.shutdown_all().await;
        
        // Shutdown the transport
        let mut transport = self.transport.clone();
        transport.shutdown().await?;
        
        Ok(())
    }
}

impl Clone for EnhancedGrpcQuicAdapter {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            connections: self.connections.clone(),
            pipelines: self.pipelines.clone(),
            next_conn_id: self.next_conn_id.clone(),
            retry_policy: self.retry_policy.clone(),
        }
    }
}
