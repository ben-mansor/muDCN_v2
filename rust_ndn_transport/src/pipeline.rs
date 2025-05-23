// μDCN Interest Pipelining Implementation
//
// This module implements Interest pipelining, which allows multiple Interest
// packets to be sent concurrently over a single QUIC connection.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Mutex, oneshot, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};
use crate::ndn::{Data, Interest};
use crate::quic_transport::{QuicTransport, ConnectionTracker};

/// Configuration for the Interest pipeline
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Maximum number of concurrent Interests in flight
    pub max_in_flight: usize,
    
    /// Interest timeout in milliseconds
    pub interest_timeout_ms: u64,
    
    /// Maximum pipeline queue size
    pub max_queue_size: usize,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            max_in_flight: 16,
            interest_timeout_ms: 4000,
            max_queue_size: 1000,
        }
    }
}

/// Request for sending an Interest via the pipeline
#[derive(Debug)]
struct PipelineRequest {
    /// Interest to send
    interest: Interest,
    
    /// Response channel
    response_tx: oneshot::Sender<Result<Data>>,
}

/// Interest pipeline for a single QUIC connection
#[derive(Debug)]
pub struct InterestPipeline {
    /// QUIC transport
    transport: Arc<QuicTransport>,
    
    /// Remote address
    remote_addr: SocketAddr,
    
    /// Pipeline configuration
    config: PipelineConfig,
    
    /// Request channel
    request_tx: mpsc::Sender<PipelineRequest>,
    
    /// Pipeline worker task
    worker_handle: Mutex<Option<JoinHandle<()>>>,
    
    /// Pipeline statistics
    stats: Arc<RwLock<PipelineStats>>,
}

/// Statistics for the Interest pipeline
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// Number of Interests sent
    pub interests_sent: u64,
    
    /// Number of Data packets received
    pub data_received: u64,
    
    /// Number of timeouts
    pub timeouts: u64,
    
    /// Number of errors
    pub errors: u64,
    
    /// Average RTT in milliseconds
    pub avg_rtt_ms: u64,
    
    /// Current queue size
    pub queue_size: usize,
    
    /// Current in-flight count
    pub in_flight: usize,
}

impl InterestPipeline {
    /// Create a new Interest pipeline for a connection
    pub fn new(
        transport: Arc<QuicTransport>,
        remote_addr: SocketAddr,
        config: PipelineConfig,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::channel(config.max_queue_size);
        let stats = Arc::new(RwLock::new(PipelineStats::default()));
        
        let mut pipeline = Self {
            transport,
            remote_addr,
            config,
            request_tx,
            worker_handle: Mutex::new(None),
            stats,
        };
        
        // Start the pipeline worker
        pipeline.start_worker(request_rx);
        
        pipeline
    }
    
    /// Start the pipeline worker task
    fn start_worker(&mut self, mut request_rx: mpsc::Receiver<PipelineRequest>) {
        let transport = self.transport.clone();
        let remote_addr = self.remote_addr;
        let config = self.config.clone();
        let stats = self.stats.clone();
        
        let handle = tokio::spawn(async move {
            // In-flight requests
            let mut in_flight: HashMap<String, PipelineRequest> = HashMap::new();
            
            // Process requests
            loop {
                // Process as many requests as we can (up to max_in_flight)
                while in_flight.len() < config.max_in_flight {
                    match request_rx.try_recv() {
                        Ok(request) => {
                            // Get the Interest name
                            let name = request.interest.name().to_string();
                            debug!("Pipelining Interest for {}", name);
                            
                            // Add to in-flight map
                            in_flight.insert(name.clone(), request);
                            
                            // Update stats
                            {
                                let mut stats = stats.write().await;
                                stats.in_flight = in_flight.len();
                                stats.interests_sent += 1;
                            }
                            
                            // Send the Interest asynchronously
                            let transport_clone = transport.clone();
                            let remote_addr_clone = remote_addr;
                            let interest_name = name.clone();
                            let stats_clone = stats.clone();
                            let timeout = Duration::from_millis(config.interest_timeout_ms);
                            
                            tokio::spawn(async move {
                                let start_time = Instant::now();
                                
                                // Send Interest with timeout
                                let result = tokio::time::timeout(
                                    timeout,
                                    transport_clone.send_interest(remote_addr_clone, in_flight[&interest_name].interest.clone())
                                ).await;
                                
                                // Calculate RTT
                                let rtt = start_time.elapsed().as_millis() as u64;
                                
                                // Process result
                                match result {
                                    Ok(Ok(data)) => {
                                        // Interest succeeded
                                        debug!("Received Data for {}, RTT: {}ms", interest_name, rtt);
                                        
                                        // Update stats
                                        {
                                            let mut stats = stats_clone.write().await;
                                            stats.data_received += 1;
                                            stats.in_flight -= 1;
                                            
                                            // Update average RTT
                                            if stats.avg_rtt_ms == 0 {
                                                stats.avg_rtt_ms = rtt;
                                            } else {
                                                // Moving average with 0.9 weight to existing average
                                                stats.avg_rtt_ms = ((stats.avg_rtt_ms as f64 * 0.9) + (rtt as f64 * 0.1)) as u64;
                                            }
                                        }
                                        
                                        // Send Data to requester
                                        if let Some(request) = in_flight.remove(&interest_name) {
                                            let _ = request.response_tx.send(Ok(data));
                                        }
                                    },
                                    Ok(Err(e)) => {
                                        // Interest failed
                                        error!("Error sending Interest for {}: {}", interest_name, e);
                                        
                                        // Update stats
                                        {
                                            let mut stats = stats_clone.write().await;
                                            stats.errors += 1;
                                            stats.in_flight -= 1;
                                        }
                                        
                                        // Send error to requester
                                        if let Some(request) = in_flight.remove(&interest_name) {
                                            let _ = request.response_tx.send(Err(e));
                                        }
                                    },
                                    Err(_) => {
                                        // Timeout
                                        warn!("Timeout sending Interest for {}", interest_name);
                                        
                                        // Update stats
                                        {
                                            let mut stats = stats_clone.write().await;
                                            stats.timeouts += 1;
                                            stats.in_flight -= 1;
                                        }
                                        
                                        // Send timeout error to requester
                                        if let Some(request) = in_flight.remove(&interest_name) {
                                            let _ = request.response_tx.send(Err(
                                                Error::Timeout(format!("Interest for {} timed out after {}ms", interest_name, timeout.as_millis()))
                                            ));
                                        }
                                    }
                                }
                            });
                        },
                        Err(mpsc::error::TryRecvError::Empty) => {
                            // No more requests to process
                            break;
                        },
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            // Channel closed, exit the worker
                            return;
                        }
                    }
                }
                
                // Wait a bit before checking for more requests
                tokio::time::sleep(Duration::from_millis(1)).await;
                
                // Update queue size in stats
                {
                    let mut stats = stats.write().await;
                    stats.queue_size = request_rx.capacity().unwrap_or(0) - request_rx.capacity().unwrap_or(0);
                }
            }
        });
        
        let mut worker_handle = self.worker_handle.blocking_lock();
        *worker_handle = Some(handle);
    }
    
    /// Send an Interest via the pipeline
    pub async fn send_interest(&self, interest: Interest) -> Result<Data> {
        // Create response channel
        let (response_tx, response_rx) = oneshot::channel();
        
        // Create pipeline request
        let request = PipelineRequest {
            interest,
            response_tx,
        };
        
        // Send request to pipeline
        self.request_tx.send(request).await.map_err(|_| {
            Error::Other("Pipeline has been shut down".to_string())
        })?;
        
        // Wait for response
        response_rx.await.map_err(|_| {
            Error::Other("Pipeline worker task has been terminated".to_string())
        })?
    }
    
    /// Get pipeline statistics
    pub async fn stats(&self) -> PipelineStats {
        self.stats.read().await.clone()
    }
    
    /// Shut down the pipeline
    pub async fn shutdown(&self) {
        // Acquire the worker handle
        let mut worker_handle = self.worker_handle.lock().await;
        
        // Abort the worker task if it's running
        if let Some(handle) = worker_handle.take() {
            handle.abort();
        }
    }
}

/// Pipeline registry for managing multiple QUIC connections
#[derive(Debug)]
pub struct PipelineRegistry {
    /// QUIC transport
    transport: Arc<QuicTransport>,
    
    /// Pipelines by connection ID
    pipelines: Arc<RwLock<HashMap<String, Arc<InterestPipeline>>>>,
    
    /// Default pipeline configuration
    default_config: PipelineConfig,
}

impl PipelineRegistry {
    /// Create a new pipeline registry
    pub fn new(transport: Arc<QuicTransport>) -> Self {
        Self {
            transport,
            pipelines: Arc::new(RwLock::new(HashMap::new())),
            default_config: PipelineConfig::default(),
        }
    }
    
    /// Set the default pipeline configuration
    pub fn set_default_config(&mut self, config: PipelineConfig) {
        self.default_config = config;
    }
    
    /// Get or create a pipeline for a connection
    pub async fn get_or_create_pipeline(&self, connection_id: &str, remote_addr: SocketAddr) -> Result<Arc<InterestPipeline>> {
        // Check if we already have a pipeline for this connection
        {
            let pipelines = self.pipelines.read().await;
            if let Some(pipeline) = pipelines.get(connection_id) {
                return Ok(pipeline.clone());
            }
        }
        
        // Create a new pipeline
        let pipeline = Arc::new(InterestPipeline::new(
            self.transport.clone(),
            remote_addr,
            self.default_config.clone(),
        ));
        
        // Register the pipeline
        {
            let mut pipelines = self.pipelines.write().await;
            pipelines.insert(connection_id.to_string(), pipeline.clone());
        }
        
        Ok(pipeline)
    }
    
    /// Remove a pipeline for a connection
    pub async fn remove_pipeline(&self, connection_id: &str) -> Result<()> {
        let mut pipelines = self.pipelines.write().await;
        
        if let Some(pipeline) = pipelines.remove(connection_id) {
            // Shut down the pipeline
            pipeline.shutdown().await;
            Ok(())
        } else {
            Err(Error::NotFound(format!("Pipeline for connection {} not found", connection_id)))
        }
    }
    
    /// Get pipeline statistics for a connection
    pub async fn get_pipeline_stats(&self, connection_id: &str) -> Result<PipelineStats> {
        let pipelines = self.pipelines.read().await;
        
        if let Some(pipeline) = pipelines.get(connection_id) {
            Ok(pipeline.stats().await)
        } else {
            Err(Error::NotFound(format!("Pipeline for connection {} not found", connection_id)))
        }
    }
    
    /// Shut down all pipelines
    pub async fn shutdown_all(&self) {
        let pipelines = {
            let mut pipelines_map = self.pipelines.write().await;
            std::mem::take(&mut *pipelines_map)
        };
        
        // Shut down each pipeline
        for (_, pipeline) in pipelines {
            pipeline.shutdown().await;
        }
    }
}
