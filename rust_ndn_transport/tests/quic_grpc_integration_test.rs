// QUIC-gRPC Integration Test
//
// This test verifies the integration between the gRPC server and QUIC transport
// by sending Interest packets via gRPC, forwarding them over QUIC, and returning
// Data responses back to the gRPC client.

use rust_ndn_transport::grpc::udcn::{
    udcn_control_client::UdcnControlClient,
    InterestPacketRequest, QuicConnectionRequest,
};
use rust_ndn_transport::grpc_quic_integration::GrpcQuicAdapter;
use rust_ndn_transport::UdcnTransport;
use rust_ndn_transport::ndn::{Interest, Data};
use rust_ndn_transport::name::Name;
use rust_ndn_transport::error::Result;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tonic::transport::Channel;
use tonic::Request;

const GRPC_PORT: u16 = 9090;
const QUIC_SERVER_PORT: u16 = 9000;
const QUIC_CLIENT_PORT: u16 = 9001;

#[tokio::test]
async fn test_grpc_quic_integration() -> Result<()> {
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
    
    // Create and start the gRPC server with QUIC adapter
    let transport = Arc::new(UdcnTransport::new()?);
    let quic_adapter = Arc::new(GrpcQuicAdapter::new("127.0.0.1", QUIC_CLIENT_PORT).await?);
    
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
    
    println!("Creating QUIC connection via gRPC");
    let connection_response = client.create_quic_connection(connection_request).await?;
    let connection_id = connection_response.into_inner().connection_id;
    println!("QUIC connection created: {}", connection_id);
    
    // Send an Interest via gRPC, which will be forwarded over QUIC
    let interest_request = Request::new(InterestPacketRequest {
        connection_id: connection_id.clone(),
        name: "/test/data".to_string(),
        can_be_prefix: false,
        must_be_fresh: true,
        interest_lifetime_ms: 5000,
        hop_limit: 255,
        forwarding_hint: Vec::new(),
        application_parameters: Vec::new(),
    });
    
    println!("Sending Interest via gRPC");
    let response = client.send_interest(interest_request).await?;
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
        "This is test data from QUIC server", 
        "Incorrect content in Data response"
    );
    
    // Close the connection
    let close_request = Request::new(rust_ndn_transport::grpc::udcn::ConnectionCloseRequest {
        connection_id,
    });
    
    println!("Closing QUIC connection");
    client.close_quic_connection(close_request).await?;
    
    // Clean up and shutdown
    quic_server_handle.abort();
    grpc_server_handle.abort();
    
    println!("Test completed successfully");
    Ok(())
}
