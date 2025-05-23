#!/usr/bin/env python3
"""
μDCN Benchmark Results Summary

This script generates a detailed summary of the benchmark results from extended test runs,
providing key metrics and statistics about each benchmark scenario.
"""

import os
import glob
import pandas as pd
import numpy as np
from datetime import datetime

# Configuration
METRICS_DIR = "/app/metrics"
SUMMARY_FILE = "/app/metrics/benchmark_summary.txt"

def load_server_metrics():
    """Load server metrics from CSV file"""
    server_file = os.path.join(METRICS_DIR, "server_metrics.csv")
    if not os.path.exists(server_file):
        print(f"Server metrics file not found: {server_file}")
        return None
    
    return pd.read_csv(server_file)

def load_client_metrics():
    """Load all client metrics and combine them"""
    # Find all client metrics files (container_id_metrics.csv)
    client_files = glob.glob(os.path.join(METRICS_DIR, "*_metrics.csv"))
    # Filter out server_metrics.csv
    client_files = [f for f in client_files if not os.path.basename(f).startswith("server")]
    
    if not client_files:
        print("No client metrics files found")
        return None
    
    # Load data and add file info
    client_data = {}
    for file in client_files:
        client_id = os.path.basename(file).split('_')[0]
        try:
            df = pd.read_csv(file)
            df['client_id'] = client_id
            
            # Extract benchmark type from interest_name
            df['benchmark_type'] = df['interest_name'].apply(
                lambda x: x.split('/')[2] if isinstance(x, str) and len(x.split('/')) > 2 else 'unknown'
            )
            
            client_data[client_id] = df
        except Exception as e:
            print(f"Error loading {file}: {e}")
    
    return client_data

def summarize_results(server_df, client_data):
    """Generate a comprehensive summary of benchmark results"""
    summary = []
    summary.append("=" * 80)
    summary.append("μDCN BENCHMARK RESULTS SUMMARY")
    summary.append("=" * 80)
    summary.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    summary.append(f"Total Clients: {len(client_data)}")
    summary.append("")
    
    # Server statistics
    if server_df is not None:
        summary.append("-" * 80)
        summary.append("SERVER PERFORMANCE METRICS")
        summary.append("-" * 80)
        
        total_cache_hits = server_df['cache_hits'].sum()
        total_cache_misses = server_df['cache_misses'].sum()
        overall_hit_rate = total_cache_hits / (total_cache_hits + total_cache_misses) if (total_cache_hits + total_cache_misses) > 0 else 0
        
        summary.append(f"Total Cache Hits: {total_cache_hits}")
        summary.append(f"Total Cache Misses: {total_cache_misses}")
        summary.append(f"Overall Cache Hit Rate: {overall_hit_rate:.2%}")
        summary.append(f"Average CPU Usage: {server_df['cpu_usage'].mean():.2f}%")
        summary.append(f"Peak CPU Usage: {server_df['cpu_usage'].max():.2f}%")
        summary.append(f"Average Memory Usage: {server_df['memory_usage'].mean():.2f} MB")
        summary.append(f"Total MTU Predictions: {server_df['mtu_predictions'].sum()}")
        summary.append(f"Total Packet Drops: {server_df['packet_drops'].sum()}")
        summary.append(f"Duration: {(server_df['timestamp'].max() - server_df['timestamp'].min()) / 1000:.2f} seconds")
        summary.append("")
    
    # Summarize each benchmark type across clients
    benchmark_types = set()
    for client_id, df in client_data.items():
        benchmark_types.update(df['benchmark_type'].unique())
    
    for benchmark_type in sorted(benchmark_types):
        if benchmark_type == 'unknown':
            continue
            
        summary.append("-" * 80)
        summary.append(f"BENCHMARK TYPE: {benchmark_type.upper()}")
        summary.append("-" * 80)
        
        # Collect metrics for this benchmark type across all clients
        total_packets = 0
        successful_packets = 0
        total_rtt = 0
        rtt_values = []
        
        for client_id, df in client_data.items():
            benchmark_df = df[df['benchmark_type'] == benchmark_type]
            if len(benchmark_df) == 0:
                continue
                
            total_packets += len(benchmark_df)
            successful_df = benchmark_df[benchmark_df['success'] == 1]
            successful_packets += len(successful_df)
            
            if len(successful_df) > 0:
                total_rtt += successful_df['rtt_ms'].sum()
                rtt_values.extend(successful_df['rtt_ms'].tolist())
        
        if total_packets > 0:
            avg_rtt = total_rtt / len(rtt_values) if len(rtt_values) > 0 else 0
            
            summary.append(f"Total Interest Packets: {total_packets}")
            summary.append(f"Successful Responses: {successful_packets}")
            summary.append(f"Success Rate: {successful_packets / total_packets:.2%}")
            
            if len(rtt_values) > 0:
                summary.append(f"Minimum RTT: {min(rtt_values):.2f} ms")
                summary.append(f"Average RTT: {avg_rtt:.2f} ms")
                summary.append(f"Maximum RTT: {max(rtt_values):.2f} ms")
                summary.append(f"RTT 95th Percentile: {np.percentile(rtt_values, 95):.2f} ms")
                summary.append(f"RTT Standard Deviation: {np.std(rtt_values):.2f} ms")
            
            summary.append("")
    
    # Client-specific metrics
    for client_id, df in client_data.items():
        summary.append("-" * 80)
        summary.append(f"CLIENT: {client_id}")
        summary.append("-" * 80)
        
        benchmark_types = df['benchmark_type'].unique()
        summary.append(f"Benchmark Types: {', '.join(benchmark_types)}")
        summary.append(f"Total Interest Packets: {len(df)}")
        
        # Packet success rate
        successful = df[df['success'] == 1]
        success_rate = len(successful) / len(df) if len(df) > 0 else 0
        summary.append(f"Success Rate: {success_rate:.2%}")
        
        # RTT statistics (if available)
        if len(successful) > 0:
            summary.append(f"Average RTT: {successful['rtt_ms'].mean():.2f} ms")
            summary.append(f"RTT 95th Percentile: {np.percentile(successful['rtt_ms'], 95):.2f} ms")
        
        # Duration
        if len(df) > 0:
            duration = (df['timestamp'].max() - df['timestamp'].min()) / 1000
            summary.append(f"Test Duration: {duration:.2f} seconds")
            summary.append(f"Effective Interest Rate: {len(df) / duration:.2f} interests/sec")
        
        # MTU statistics (if available)
        if 'measured_mtu' in df.columns:
            mtu_measurements = df[df['measured_mtu'] > 0]
            if len(mtu_measurements) > 0:
                summary.append(f"MTU Measurements: {len(mtu_measurements)}")
                summary.append(f"Average MTU: {mtu_measurements['measured_mtu'].mean():.2f} bytes")
        
        summary.append("")
    
    return "\n".join(summary)

def main():
    print("Loading server metrics...")
    server_df = load_server_metrics()
    
    print("Loading client metrics...")
    client_data = load_client_metrics()
    
    if client_data:
        print("Generating benchmark summary...")
        summary = summarize_results(server_df, client_data)
        
        # Save summary to file
        with open(SUMMARY_FILE, 'w') as f:
            f.write(summary)
        
        print(f"Summary saved to {SUMMARY_FILE}")
        print(summary)
    else:
        print("No client data available for summary")

if __name__ == "__main__":
    main()
