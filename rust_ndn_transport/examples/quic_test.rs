// QUIC Transport Test CLI
//
// This example showcases a simple CLI to test the QUIC transport
// layer for NDN over QUIC. It can run in server or client mode.

use clap::{App, Arg, SubCommand};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::signal;

use rust_ndn_transport::error::Result;
use rust_ndn_transport::ndn::{Data, Interest};
use rust_ndn_transport::name::Name;
use rust_ndn_transport::quic_transport::QuicTransport;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Parse command line arguments
    let matches = App::new("QUIC NDN Transport Test")
        .version("0.1.0")
        .author("μDCN Team")
        .about("Test utility for NDN over QUIC")
        .subcommand(
            SubCommand::with_name("server")
                .about("Run in server mode")
                .arg(
                    Arg::with_name("bind")
                        .short("b")
                        .long("bind")
                        .value_name("ADDRESS")
                        .help("Address to bind to")
                        .default_value("127.0.0.1")
                )
                .arg(
                    Arg::with_name("port")
                        .short("p")
                        .long("port")
                        .value_name("PORT")
                        .help("Port to listen on")
                        .default_value("9000")
                )
        )
        .subcommand(
            SubCommand::with_name("client")
                .about("Run in client mode")
                .arg(
                    Arg::with_name("server")
                        .short("s")
                        .long("server")
                        .value_name("ADDRESS")
                        .help("Server address")
                        .default_value("127.0.0.1")
                )
                .arg(
                    Arg::with_name("port")
                        .short("p")
                        .long("port")
                        .value_name("PORT")
                        .help("Server port")
                        .default_value("9000")
                )
                .arg(
                    Arg::with_name("name")
                        .short("n")
                        .long("name")
                        .value_name("NAME")
                        .help("NDN name to request")
                        .default_value("/test/data")
                )
                .arg(
                    Arg::with_name("count")
                        .short("c")
                        .long("count")
                        .value_name("COUNT")
                        .help("Number of Interest packets to send")
                        .default_value("1")
                )
        )
        .get_matches();
    
    // Handle subcommands
    if let Some(matches) = matches.subcommand_matches("server") {
        // Run in server mode
        let bind_addr = matches.value_of("bind").unwrap();
        let port = matches.value_of("port").unwrap().parse::<u16>().unwrap();
        
        run_server(bind_addr, port).await?;
    } else if let Some(matches) = matches.subcommand_matches("client") {
        // Run in client mode
        let server_addr = matches.value_of("server").unwrap();
        let port = matches.value_of("port").unwrap().parse::<u16>().unwrap();
        let name = matches.value_of("name").unwrap();
        let count = matches.value_of("count").unwrap().parse::<u32>().unwrap();
        
        run_client(server_addr, port, name, count).await?;
    } else {
        println!("Please specify either 'server' or 'client' mode. Use --help for more information.");
    }
    
    Ok(())
}

/// Run the QUIC transport in server mode
async fn run_server(bind_addr: &str, port: u16) -> Result<()> {
    println!("Starting QUIC NDN server on {}:{}", bind_addr, port);
    
    // Create transport instance
    let mut transport = QuicTransport::new(bind_addr, port, 30, 65536).await?;
    
    // Register handler for all Interests starting with '/test'
    transport.register_handler(Name::from("/test"), |interest| {
        println!("Received Interest: {}", interest.name());
        
        // Create a Data packet with some content
        let content = format!("Response data for {}", interest.name()).into_bytes();
        let data = Data::new(interest.name().clone(), content);
        
        Ok(data)
    }).await?;
    
    // Start the transport server
    transport.start_server().await?;
    println!("Server started, press Ctrl+C to exit");
    
    // Wait for Ctrl+C
    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    
    // Shutdown
    println!("Shutting down server...");
    transport.shutdown().await?;
    
    Ok(())
}

/// Run the QUIC transport in client mode
async fn run_client(server_addr: &str, port: u16, name_str: &str, count: u32) -> Result<()> {
    println!("Starting QUIC NDN client to connect to {}:{}", server_addr, port);
    
    // Create transport instance (binding to any port)
    let transport = QuicTransport::new("0.0.0.0", 0, 30, 65536).await?;
    
    // Connect to the server
    let conn_tracker = transport.connect(server_addr, port).await?;
    println!("Connected to server at {}:{}", server_addr, port);
    
    // Parse the name
    let name = Name::from(name_str);
    
    // Send Interest packets
    for i in 0..count {
        println!("Sending Interest #{} for {}", i+1, name);
        
        // Create Interest
        let interest = Interest::new(name.clone());
        
        // Send Interest and wait for Data
        match transport.send_interest(*conn_tracker.remote_addr(), interest).await {
            Ok(data) => {
                let content = String::from_utf8_lossy(&data.content());
                println!("Received Data: {} with content: {}", data.name(), content);
                
                // Get statistics
                if let Some(stats) = transport.get_connection_stats(*conn_tracker.remote_addr()).await {
                    println!("Connection stats - RTT: {}ms, Data received: {}", 
                             stats.rtt_ms, stats.data_received);
                }
            },
            Err(e) => {
                eprintln!("Error sending Interest: {}", e);
            }
        }
    }
    
    // Close the connection
    transport.close_connection(*conn_tracker.remote_addr()).await?;
    println!("Connection closed");
    
    Ok(())
}
