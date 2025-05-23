#!/usr/bin/env python3
"""
μDCN Benchmarking Metrics Analysis

This script processes metrics collected during μDCN benchmarks
and generates visualizations for performance analysis.
"""

import os
import glob
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
from datetime import datetime
import json

# Configuration
METRICS_DIR = "/app/metrics"
OUTPUT_DIR = "/app/benchmark"
PLOT_DPI = 300

def load_server_metrics():
    """Load server metrics from CSV file"""
    server_file = os.path.join(METRICS_DIR, "server_metrics.csv")
    if not os.path.exists(server_file):
        print(f"Server metrics file not found: {server_file}")
        return None
    
    return pd.read_csv(server_file)

def load_client_metrics():
    """Load all client metrics and combine them"""
    client_files = glob.glob(os.path.join(METRICS_DIR, "udcn_client*_metrics.csv"))
    if not client_files:
        print("No client metrics files found")
        return None
    
    # Load all client data
    dfs = []
    for file in client_files:
        client_id = os.path.basename(file).split('_')[0]
        df = pd.read_csv(file)
        df['client_id'] = client_id
        dfs.append(df)
    
    return pd.concat(dfs, ignore_index=True)

def preprocess_metrics(server_df, client_df):
    """Preprocess metrics for analysis"""
    # Convert timestamp to datetime
    if server_df is not None:
        server_df['datetime'] = pd.to_datetime(server_df['timestamp'], unit='ms')
    
    if client_df is not None:
        client_df['datetime'] = pd.to_datetime(client_df['timestamp'], unit='ms')
        
        # Extract benchmark type from interest_name
        client_df['benchmark_type'] = client_df['interest_name'].apply(
            lambda x: x.split('/')[2] if len(x.split('/')) > 2 else 'unknown'
        )
        
        # Extract size from MTU test interest names
        def extract_size(name):
            if 'size=' in name:
                return int(name.split('size=')[1])
            return 0
        
        client_df['requested_size'] = client_df['interest_name'].apply(extract_size)
    
    return server_df, client_df

def plot_cache_performance(server_df):
    """Plot cache hit ratio over time"""
    if server_df is None:
        return
    
    plt.figure(figsize=(10, 6))
    plt.plot(server_df['datetime'], server_df['cache_hit_ratio'], 'b-', label='Cache Hit Ratio')
    plt.fill_between(server_df['datetime'], 0, server_df['cache_hit_ratio'], alpha=0.3)
    
    # Add hit/miss counts
    plt.plot(server_df['datetime'], server_df['cache_hits'] / 
             server_df[['cache_hits', 'cache_misses']].max().max(), 
             'g--', label='Cache Hits (normalized)')
    plt.plot(server_df['datetime'], server_df['cache_misses'] / 
             server_df[['cache_hits', 'cache_misses']].max().max(), 
             'r--', label='Cache Misses (normalized)')
    
    plt.title('Cache Performance Over Time')
    plt.xlabel('Time')
    plt.ylabel('Ratio / Normalized Count')
    plt.grid(True, alpha=0.3)
    plt.legend()
    plt.tight_layout()
    
    # Save plot
    plt.savefig(os.path.join(OUTPUT_DIR, 'cache_performance.png'), dpi=PLOT_DPI)
    plt.close()

def plot_client_latency(client_df):
    """Plot client latency distributions by benchmark type"""
    if client_df is None:
        return
    
    benchmark_types = client_df['benchmark_type'].unique()
    
    plt.figure(figsize=(12, 8))
    
    for i, benchmark in enumerate(benchmark_types):
        subset = client_df[client_df['benchmark_type'] == benchmark]
        
        if subset.empty:
            continue
            
        # Only show successful requests
        successful = subset[subset['success'] == 1]
        if successful.empty:
            continue
            
        plt.subplot(len(benchmark_types), 1, i+1)
        
        # Plot histogram of RTTs
        plt.hist(successful['rtt_ms'], bins=50, alpha=0.7, 
                 label=f'{benchmark} (n={len(successful)})')
        
        plt.axvline(successful['rtt_ms'].mean(), color='r', linestyle='dashed', 
                    linewidth=1, label=f'Mean: {successful["rtt_ms"].mean():.2f}ms')
        
        plt.title(f'RTT Distribution for {benchmark}')
        plt.xlabel('RTT (ms)')
        plt.ylabel('Count')
        plt.grid(True, alpha=0.3)
        plt.legend()
    
    plt.tight_layout()
    plt.savefig(os.path.join(OUTPUT_DIR, 'client_latency.png'), dpi=PLOT_DPI)
    plt.close()

def plot_mtu_prediction(client_df):
    """Plot MTU predictions vs requested sizes"""
    if client_df is None:
        return
    
    # Filter for MTU prediction tests
    mtu_df = client_df[client_df['benchmark_type'] == 'mtu']
    if mtu_df.empty:
        return
    
    # Filter for successful requests
    mtu_df = mtu_df[mtu_df['success'] == 1]
    if mtu_df.empty:
        return
    
    # Group by requested size
    size_groups = mtu_df.groupby('requested_size')
    
    plt.figure(figsize=(10, 6))
    
    # Plot actual size vs requested size
    sizes = []
    mtus = []
    stds = []
    
    for size, group in size_groups:
        if size > 0:  # Skip entries without size info
            sizes.append(size)
            mtus.append(group['measured_mtu'].mean())
            stds.append(group['measured_mtu'].std())
    
    # Plot
    plt.errorbar(sizes, mtus, yerr=stds, fmt='o-', capsize=5, 
                 label='Measured MTU with std dev')
    
    # Add ideal line
    max_size = max(sizes)
    plt.plot([0, max_size], [0, max_size], 'k--', alpha=0.5, 
             label='Ideal (MTU = Requested Size)')
    
    plt.title('MTU Predictions vs Requested Sizes')
    plt.xlabel('Requested Size (bytes)')
    plt.ylabel('Measured MTU (bytes)')
    plt.grid(True, alpha=0.3)
    plt.legend()
    plt.tight_layout()
    
    plt.savefig(os.path.join(OUTPUT_DIR, 'mtu_prediction.png'), dpi=PLOT_DPI)
    plt.close()

def plot_cache_warmup_comparison(client_df):
    """Compare performance between cold and warm cache"""
    if client_df is None:
        return
    
    # Filter for cache test
    cache_df = client_df[client_df['benchmark_type'] == 'cache']
    if cache_df.empty:
        return
    
    # Check if we have cache state info
    if 'cache_state' not in cache_df.columns and len(cache_df) > 0:
        # Try to infer state from the last column
        last_col = cache_df.columns[-1]
        if cache_df[last_col].isin(['cold', 'warm']).any():
            cache_df['cache_state'] = cache_df[last_col]
        else:
            return
    
    # Prepare data for successful requests
    successful = cache_df[cache_df['success'] == 1]
    if successful.empty:
        return
    
    # Group by cache state
    cold_cache = successful[successful['cache_state'] == 'cold']
    warm_cache = successful[successful['cache_state'] == 'warm']
    
    if cold_cache.empty or warm_cache.empty:
        return
    
    # Prepare plot
    plt.figure(figsize=(12, 10))
    
    # Plot 1: RTT comparison
    plt.subplot(2, 1, 1)
    labels = ['Cold Cache', 'Warm Cache']
    rtts = [cold_cache['rtt_ms'].mean(), warm_cache['rtt_ms'].mean()]
    rtt_stds = [cold_cache['rtt_ms'].std(), warm_cache['rtt_ms'].std()]
    
    plt.bar(labels, rtts, yerr=rtt_stds, capsize=5, alpha=0.7)
    plt.title('Average RTT: Cold vs Warm Cache')
    plt.ylabel('RTT (ms)')
    plt.grid(True, alpha=0.3)
    
    # Add text labels
    for i, v in enumerate(rtts):
        plt.text(i, v + rtt_stds[i] + 5, f'{v:.2f}ms', 
                 ha='center', va='bottom', fontweight='bold')
    
    # Plot 2: RTT distributions
    plt.subplot(2, 1, 2)
    plt.hist(cold_cache['rtt_ms'], bins=30, alpha=0.5, label='Cold Cache')
    plt.hist(warm_cache['rtt_ms'], bins=30, alpha=0.5, label='Warm Cache')
    plt.title('RTT Distribution: Cold vs Warm Cache')
    plt.xlabel('RTT (ms)')
    plt.ylabel('Count')
    plt.grid(True, alpha=0.3)
    plt.legend()
    
    plt.tight_layout()
    plt.savefig(os.path.join(OUTPUT_DIR, 'cache_warmup_comparison.png'), dpi=PLOT_DPI)
    plt.close()

def plot_controller_fallback(client_df):
    """Analyze controller fallback performance"""
    if client_df is None:
        return
    
    # Filter for fallback test
    fallback_df = client_df[client_df['benchmark_type'] == 'fallback']
    if fallback_df.empty:
        return
    
    # Check if we have fallback state info (normal vs fallback)
    if 'fallback_state' not in fallback_df.columns and len(fallback_df) > 0:
        # Try to infer state from the last column
        last_col = fallback_df.columns[-1]
        if fallback_df[last_col].isin(['normal', 'fallback']).any():
            fallback_df['fallback_state'] = fallback_df[last_col]
        else:
            return
    
    # Analyze success rates
    normal_requests = fallback_df[fallback_df['fallback_state'] == 'normal']
    fallback_requests = fallback_df[fallback_df['fallback_state'] == 'fallback']
    
    if normal_requests.empty or fallback_requests.empty:
        return
    
    normal_success_rate = normal_requests['success'].mean() * 100
    fallback_success_rate = fallback_requests['success'].mean() * 100
    
    # Plot success rates
    plt.figure(figsize=(10, 12))
    
    # Plot 1: Success rates
    plt.subplot(3, 1, 1)
    labels = ['Normal Operation', 'Fallback Mode']
    success_rates = [normal_success_rate, fallback_success_rate]
    
    plt.bar(labels, success_rates, alpha=0.7)
    plt.title('Success Rate: Normal vs Fallback Mode')
    plt.ylabel('Success Rate (%)')
    plt.ylim(0, 105)  # Leave room for text
    plt.grid(True, alpha=0.3)
    
    # Add text labels
    for i, v in enumerate(success_rates):
        plt.text(i, v + 2, f'{v:.1f}%', ha='center', va='bottom', fontweight='bold')
    
    # Plot 2: RTT comparison for successful requests
    plt.subplot(3, 1, 2)
    
    normal_successful = normal_requests[normal_requests['success'] == 1]
    fallback_successful = fallback_requests[fallback_requests['success'] == 1]
    
    if not normal_successful.empty and not fallback_successful.empty:
        labels = ['Normal Operation', 'Fallback Mode']
        rtts = [normal_successful['rtt_ms'].mean(), fallback_successful['rtt_ms'].mean()]
        rtt_stds = [normal_successful['rtt_ms'].std(), fallback_successful['rtt_ms'].std()]
        
        plt.bar(labels, rtts, yerr=rtt_stds, capsize=5, alpha=0.7)
        plt.title('Average RTT for Successful Requests')
        plt.ylabel('RTT (ms)')
        plt.grid(True, alpha=0.3)
        
        # Add text labels
        for i, v in enumerate(rtts):
            plt.text(i, v + rtt_stds[i] + 5, f'{v:.2f}ms', 
                    ha='center', va='bottom', fontweight='bold')
    
    # Plot 3: Success over time
    plt.subplot(3, 1, 3)
    
    # Group by time windows
    normal_requests['time_window'] = (normal_requests['timestamp'] - 
                                     normal_requests['timestamp'].min()) // 5000
    fallback_requests['time_window'] = (fallback_requests['timestamp'] - 
                                       fallback_requests['timestamp'].min()) // 5000
    
    normal_success = normal_requests.groupby('time_window')['success'].mean() * 100
    fallback_success = fallback_requests.groupby('time_window')['success'].mean() * 100
    
    plt.plot(normal_success.index, normal_success.values, 'bo-', label='Normal Operation')
    plt.plot(fallback_success.index, fallback_success.values, 'ro-', label='Fallback Mode')
    
    plt.title('Success Rate Over Time')
    plt.xlabel('Time Window (5 second intervals)')
    plt.ylabel('Success Rate (%)')
    plt.grid(True, alpha=0.3)
    plt.legend()
    
    plt.tight_layout()
    plt.savefig(os.path.join(OUTPUT_DIR, 'controller_fallback.png'), dpi=PLOT_DPI)
    plt.close()

def generate_summary_report(server_df, client_df):
    """Generate a summary report with key metrics"""
    report = {
        "timestamp": datetime.now().isoformat(),
        "server_metrics": {},
        "client_metrics": {}
    }
    
    # Server metrics
    if server_df is not None and not server_df.empty:
        report["server_metrics"] = {
            "avg_cache_hit_ratio": server_df['cache_hit_ratio'].mean(),
            "avg_latency_ms": server_df['avg_latency_ms'].mean(),
            "total_cache_hits": server_df['cache_hits'].sum(),
            "total_cache_misses": server_df['cache_misses'].sum(),
            "avg_cpu_usage": server_df['cpu_usage'].mean(),
            "avg_memory_usage_mb": server_df['memory_usage'].mean(),
            "total_mtu_predictions": server_df['mtu_predictions'].sum(),
            "total_packet_drops": server_df['packet_drops'].sum()
        }
    
    # Client metrics
    if client_df is not None and not client_df.empty:
        successful = client_df[client_df['success'] == 1]
        failed = client_df[client_df['success'] == 0]
        
        # Calculate statistics by benchmark type
        benchmark_stats = {}
        for benchmark in client_df['benchmark_type'].unique():
            benchmark_df = client_df[client_df['benchmark_type'] == benchmark]
            benchmark_success = benchmark_df[benchmark_df['success'] == 1]
            
            if not benchmark_success.empty:
                benchmark_stats[benchmark] = {
                    "requests": len(benchmark_df),
                    "success_rate": (len(benchmark_success) / len(benchmark_df)) * 100,
                    "avg_rtt_ms": benchmark_success['rtt_ms'].mean(),
                    "min_rtt_ms": benchmark_success['rtt_ms'].min(),
                    "max_rtt_ms": benchmark_success['rtt_ms'].max(),
                    "avg_data_size": benchmark_success['data_size'].mean()
                }
        
        report["client_metrics"] = {
            "total_requests": len(client_df),
            "successful_requests": len(successful),
            "failed_requests": len(failed),
            "success_rate": (len(successful) / len(client_df)) * 100 if len(client_df) > 0 else 0,
            "avg_rtt_ms": successful['rtt_ms'].mean() if not successful.empty else 0,
            "benchmark_stats": benchmark_stats
        }
    
    # Save report as JSON
    with open(os.path.join(OUTPUT_DIR, 'summary_report.json'), 'w') as f:
        json.dump(report, f, indent=2)
    
    # Also save as human-readable text
    with open(os.path.join(OUTPUT_DIR, 'summary_report.txt'), 'w') as f:
        f.write("μDCN BENCHMARK SUMMARY REPORT\n")
        f.write("============================\n")
        f.write(f"Generated: {report['timestamp']}\n\n")
        
        f.write("SERVER METRICS\n")
        f.write("-------------\n")
        for key, value in report["server_metrics"].items():
            f.write(f"{key}: {value:.2f}\n")
        
        f.write("\nCLIENT METRICS\n")
        f.write("-------------\n")
        f.write(f"Total Requests: {report['client_metrics'].get('total_requests', 0)}\n")
        f.write(f"Success Rate: {report['client_metrics'].get('success_rate', 0):.2f}%\n")
        f.write(f"Average RTT: {report['client_metrics'].get('avg_rtt_ms', 0):.2f} ms\n\n")
        
        f.write("BENCHMARK STATISTICS\n")
        f.write("-------------------\n")
        for benchmark, stats in report["client_metrics"].get("benchmark_stats", {}).items():
            f.write(f"\n{benchmark.upper()}:\n")
            for key, value in stats.items():
                f.write(f"  {key}: {value:.2f}\n")

def create_html_dashboard():
    """Create an HTML dashboard with all plots"""
    html_content = """
    <!DOCTYPE html>
    <html>
    <head>
        <title>μDCN Benchmark Results</title>
        <style>
            body { font-family: Arial, sans-serif; margin: 0; padding: 20px; background-color: #f5f5f5; }
            .container { max-width: 1200px; margin: 0 auto; background-color: white; padding: 20px; border-radius: 8px; box-shadow: 0 0 10px rgba(0,0,0,0.1); }
            h1 { color: #333; border-bottom: 2px solid #eee; padding-bottom: 10px; }
            .plot-container { margin: 20px 0; padding: 15px; border: 1px solid #eee; border-radius: 8px; }
            .plot-container h2 { margin-top: 0; color: #555; }
            img { max-width: 100%; height: auto; display: block; margin: 0 auto; }
            .summary { margin: 20px 0; padding: 15px; background-color: #f8f8f8; border-left: 4px solid #4CAF50; }
            pre { background-color: #f8f8f8; padding: 10px; overflow-x: auto; }
        </style>
    </head>
    <body>
        <div class="container">
            <h1>μDCN Benchmark Results</h1>
            
            <div class="summary">
                <h2>Summary Report</h2>
                <pre id="summary-report">Loading...</pre>
            </div>
            
            <div class="plot-container">
                <h2>Cache Performance</h2>
                <img src="cache_performance.png" alt="Cache Performance Over Time">
                <p>This chart shows the cache hit ratio over time, along with normalized counts of cache hits and misses.</p>
            </div>
            
            <div class="plot-container">
                <h2>Client Latency</h2>
                <img src="client_latency.png" alt="Client Latency Distributions">
                <p>Distribution of round-trip times (RTT) for different benchmark types.</p>
            </div>
            
            <div class="plot-container">
                <h2>MTU Prediction</h2>
                <img src="mtu_prediction.png" alt="MTU Predictions vs Requested Sizes">
                <p>Comparison of predicted MTU values against requested data sizes.</p>
            </div>
            
            <div class="plot-container">
                <h2>Cache Warmup Comparison</h2>
                <img src="cache_warmup_comparison.png" alt="Cold vs Warm Cache Performance">
                <p>Performance comparison between cold and warm cache states.</p>
            </div>
            
            <div class="plot-container">
                <h2>Controller Fallback</h2>
                <img src="controller_fallback.png" alt="Controller Fallback Performance">
                <p>Analysis of system performance during normal operation vs. fallback mode.</p>
            </div>
        </div>
        
        <script>
            // Load summary report
            fetch('summary_report.json')
                .then(response => response.json())
                .then(data => {
                    document.getElementById('summary-report').textContent = JSON.stringify(data, null, 2);
                })
                .catch(error => {
                    document.getElementById('summary-report').textContent = "Error loading report: " + error;
                });
        </script>
    </body>
    </html>
    """
    
    with open(os.path.join(OUTPUT_DIR, 'index.html'), 'w') as f:
        f.write(html_content)
    
    print(f"Dashboard created at {os.path.join(OUTPUT_DIR, 'index.html')}")

def main():
    """Main analysis function"""
    print("Starting μDCN benchmark metrics analysis...")
    
    # Create output directory if it doesn't exist
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    
    # Load metrics
    print("Loading metrics data...")
    server_df = load_server_metrics()
    client_df = load_client_metrics()
    
    if server_df is None and client_df is None:
        print("No metrics data found. Exiting.")
        return
    
    # Preprocess metrics
    print("Preprocessing metrics...")
    server_df, client_df = preprocess_metrics(server_df, client_df)
    
    # Generate plots
    print("Generating plots...")
    
    if server_df is not None:
        plot_cache_performance(server_df)
    
    if client_df is not None:
        plot_client_latency(client_df)
        plot_mtu_prediction(client_df)
        plot_cache_warmup_comparison(client_df)
        plot_controller_fallback(client_df)
    
    # Generate summary report
    print("Generating summary report...")
    generate_summary_report(server_df, client_df)
    
    # Create HTML dashboard
    print("Creating HTML dashboard...")
    create_html_dashboard()
    
    print("Analysis complete! View results in the dashboard.")

if __name__ == "__main__":
    main()
