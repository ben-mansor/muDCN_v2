// Enhanced QUIC-gRPC Integration Test
//
// This test demonstrates the improved integration between gRPC and QUIC transport
// with retry and pipelining capabilities.

use rust_ndn_transport::grpc::udcn::{
    udcn_control_client::UdcnControlClient,
    InterestPacketRequest, QuicConnectionRequest,
};
use rust_ndn_transport::grpc_quic_integration_enhanced::EnhancedGrpcQuicAdapter;
use rust_ndn_transport::UdcnTransport;
use rust_ndn_transport::ndn::{Interest, Data};
use rust_ndn_transport::name::Name;
use rust_ndn_transport::error::Result;
use rust_ndn_transport::interest_retry::RetryPolicy;
use rust_ndn_transport::pipeline::PipelineConfig;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tonic::transport::Channel;
use tonic::Request;
use futures::future::join_all;

const GRPC_PORT: u16 = 9090;
const QUIC_SERVER_PORT: u16 = 9000;
const QUIC_CLIENT_PORT: u16 = 9001;

#[tokio::test]
async fn test_enhanced_grpc_quic_integration() -> Result<()> {
    // Initialize tracing for better debugging
    tracing_subscriber::fmt::init();
    
    // Start a QUIC server that will serve Data for our test Interest
    let quic_server_handle = tokio::spawn(async move {
        println!("Starting QUIC server on port {}", QUIC_SERVER_PORT);
        
        // Create the QUIC transport server
        let quic_server = rust_ndn_transport::quic_transport::QuicTransport::new(
            "127.0.0.1",
            QUIC_SERVER_PORT,
            30,
            65535
        ).await?;
        
        // Start the server
        let mut quic_server_mut = quic_server.clone();
        quic_server_mut.start_server().await?;
        
        // Register a handler for our test Interest
        quic_server.register_handler(Name::from("/test/data"), |interest| {
            println!("QUIC server received Interest: {}", interest.name());
            
            // Simulate some processing time (varying to test retry)
            let rng = rand::random::<u8>() % 100;
            if rng < 20 {
                // 20% chance of delay
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            
            // Create Data response
            let mut data = Data::new(interest.name().clone());
            data.set_content(b"This is test data from QUIC server".to_vec());
            data.set_content_type(0); // BLOB type
            data.set_freshness_period_ms(1000);
            
            println!("QUIC server sending Data response");
            Ok(data)
        }).await?;
        
        // Keep the server running for the duration of the test
        while let Ok(_) = sleep(Duration::from_secs(1)).await {}
        
        Ok::<_, rust_ndn_transport::error::Error>(())
    });
    
    // Wait a moment for the server to start
    sleep(Duration::from_millis(500)).await;
    
    // Create and start the gRPC server with enhanced QUIC adapter
    let transport = Arc::new(UdcnTransport::new()?);
    
    // Create a retry policy that's aggressive for testing
    let retry_policy = RetryPolicy {
        max_attempts: 5,
        base_delay_ms: 50,
        max_delay_ms: 1000,
        backoff_factor: 1.5,
        with_jitter: true,
    };
    
    let quic_adapter = Arc::new(EnhancedGrpcQuicAdapter::new_with_retry_policy(
        "127.0.0.1", 
        QUIC_CLIENT_PORT,
        retry_policy
    ).await?);
    
    let grpc_server_addr = format!("127.0.0.1:{}", GRPC_PORT);
    let grpc_server_handle = tokio::spawn(async move {
        println!("Starting gRPC server on {}", grpc_server_addr);
        rust_ndn_transport::grpc::run_grpc_server(
            transport,
            grpc_server_addr,
            Some(quic_adapter)
        ).await
    });
    
    // Wait for gRPC server to start
    sleep(Duration::from_millis(500)).await;
    
    // Connect to the gRPC server as a client
    let channel = Channel::from_shared(format!("http://127.0.0.1:{}", GRPC_PORT))?
        .connect()
        .await?;
    
    let mut client = UdcnControlClient::new(channel);
    
    // First, create a QUIC connection to the server via gRPC
    let connection_request = Request::new(QuicConnectionRequest {
        peer_address: "127.0.0.1".to_string(),
        port: QUIC_SERVER_PORT as u32,
        connect_timeout_ms: 5000,
    });
    
    println!("Creating QUIC connection");
    let connection_response = client.create_quic_connection(connection_request).await?;
    let connection_id = connection_response.into_inner().connection_id;
    println!("Connection established with ID: {}", connection_id);
    
    // Now test sending multiple Interest packets in parallel to demonstrate pipelining
    let mut interest_futures = Vec::new();
    
    for i in 0..10 {
        let name = format!("/test/data/{}", i);
        let interest_request = Request::new(InterestPacketRequest {
            connection_id: connection_id.clone(),
            name,
            can_be_prefix: false,
            must_be_fresh: true,
            lifetime_ms: 1000,
        });
        
        let mut client_clone = client.clone();
        let fut = async move {
            let start = std::time::Instant::now();
            let result = client_clone.send_interest(interest_request).await;
            let elapsed = start.elapsed();
            (result, elapsed)
        };
        
        interest_futures.push(fut);
    }
    
    // Send all Interests in parallel
    println!("Sending 10 parallel Interests");
    let results = join_all(interest_futures).await;
    
    // Verify all results
    for (i, (result, elapsed)) in results.into_iter().enumerate() {
        match result {
            Ok(response) => {
                let data_response = response.into_inner();
                println!("Interest {} succeeded in {:?}, received data for: {}", 
                    i, elapsed, data_response.name);
                assert!(data_response.success, "Data response was not successful");
                assert_eq!(data_response.name, format!("/test/data/{}", i), "Incorrect name in Data response");
            },
            Err(err) => {
                panic!("Interest {} failed: {}", i, err);
            }
        }
    }
    
    // Test retry mechanism by simulating a network issue
    // In real situations, we'd actually cause network disruption, 
    // but here we'll rely on the randomized behavior in our handler
    
    println!("Testing retry mechanism with multiple requests");
    let mut retry_futures = Vec::new();
    
    // Send several requests to increase chance of hitting the delay path
    for i in 0..5 {
        let interest_request = Request::new(InterestPacketRequest {
            connection_id: connection_id.clone(),
            name: format!("/test/retry/{}", i),
            can_be_prefix: false,
            must_be_fresh: true,
            lifetime_ms: 100, // Very short timeout to test retry
        });
        
        let mut client_clone = client.clone();
        let fut = async move {
            let result = client_clone.send_interest(interest_request).await;
            (i, result)
        };
        
        retry_futures.push(fut);
    }
    
    let retry_results = join_all(retry_futures).await;
    
    // Verify retry results
    for (i, result) in retry_results {
        match result {
            Ok(response) => {
                let data_response = response.into_inner();
                println!("Retry test {} succeeded, received data for: {}", 
                    i, data_response.name);
                assert!(data_response.success, "Data response was not successful");
            },
            Err(err) => {
                panic!("Retry test {} failed: {}", i, err);
            }
        }
    }
    
    // Clean up
    println!("Test completed successfully, cleaning up");
    quic_server_handle.abort();
    grpc_server_handle.abort();
    
    Ok(())
}
