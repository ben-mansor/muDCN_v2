#!/usr/bin/env python3
"""
μDCN Benchmarking Visualization Generator

This script generates specific visualization plots from μDCN benchmark metrics:
1. Cache hit rate over time
2. End-to-end latency vs cache status
3. Predicted MTU vs RTT

Output is saved to /app/metrics/plots/ directory
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
PLOTS_DIR = "/app/metrics/plots"

# Ensure plots directory exists
os.makedirs(PLOTS_DIR, exist_ok=True)

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
    
    # Load all client data
    dfs = []
    for file in client_files:
        client_id = os.path.basename(file).split('_')[0]
        try:
            df = pd.read_csv(file)
            df['client_id'] = client_id
            dfs.append(df)
        except Exception as e:
            print(f"Error loading {file}: {e}")
    
    if not dfs:
        return None
        
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
            lambda x: x.split('/')[2] if isinstance(x, str) and len(x.split('/')) > 2 else 'unknown'
        )
        
        # Determine cache status (inferred from interest name)
        def infer_cache_status(name):
            if not isinstance(name, str):
                return 'unknown'
            if '/cache/cold/' in name:
                return 'cold'
            elif '/cache/warm/' in name:
                return 'warm'
            else:
                return 'unknown'
        
        client_df['cache_status'] = client_df['interest_name'].apply(infer_cache_status)
    
    return server_df, client_df

def plot_cache_hit_rate_over_time(server_df):
    """Plot cache hit ratio over time"""
    if server_df is None or 'cache_hit_ratio' not in server_df.columns:
        print("Cannot generate cache hit rate plot: missing data")
        return
    
    plt.figure(figsize=(12, 6))
    plt.plot(server_df['datetime'], server_df['cache_hit_ratio'], 'b-', linewidth=2, label='Cache Hit Ratio')
    plt.fill_between(server_df['datetime'], 0, server_df['cache_hit_ratio'], alpha=0.3, color='blue')
    
    # Add reference lines
    plt.axhline(y=0.5, color='gray', linestyle='--', alpha=0.7, label='50% Hit Rate')
    plt.axhline(y=0.8, color='green', linestyle='--', alpha=0.7, label='80% Hit Rate')
    
    # Format plot
    plt.title('Cache Hit Rate Over Time', fontsize=16)
    plt.xlabel('Time', fontsize=14)
    plt.ylabel('Cache Hit Ratio', fontsize=14)
    plt.ylim(0, 1)
    plt.grid(True, alpha=0.3)
    plt.legend(fontsize=12)
    plt.tight_layout()
    
    # Save plot
    plot_path = os.path.join(PLOTS_DIR, 'cache_hit_rate_over_time.png')
    plt.savefig(plot_path, dpi=300)
    print(f"Saved cache hit rate plot to {plot_path}")
    plt.close()

def plot_latency_vs_cache_status(client_df, server_df):
    """Plot end-to-end latency by cache status (hit/miss)"""
    if client_df is None or server_df is None:
        print("Cannot generate latency vs cache status plot: missing data")
        return
    
    # Merge client and server data to associate RTTs with cache status
    # For simplicity, we'll use the inferred cache status from interest names
    cache_statuses = client_df['cache_status'].unique()
    cache_statuses = [s for s in cache_statuses if s != 'unknown']
    
    if not cache_statuses:
        # Alternative approach: divide requests into hit/miss groups based on server data
        # This is synthetic but provides a visualization
        # Sort server data by hit ratio
        server_df_sorted = server_df.sort_values('cache_hit_ratio')
        
        # Create two groups - lower half (mostly misses) and upper half (mostly hits)
        midpoint = len(server_df_sorted) // 2
        
        # Get timestamps for each group
        miss_timestamps = set(server_df_sorted.iloc[:midpoint]['timestamp'])
        hit_timestamps = set(server_df_sorted.iloc[midpoint:]['timestamp'])
        
        # Assign synthetic cache status to client data
        def assign_status(timestamp):
            if timestamp in hit_timestamps:
                return 'hit (inferred)'
            elif timestamp in miss_timestamps:
                return 'miss (inferred)'
            else:
                return 'unknown'
        
        # Group client data by synthetic cache status
        client_df['synthetic_cache_status'] = client_df['timestamp'].apply(assign_status)
        cache_statuses = ['hit (inferred)', 'miss (inferred)']
        status_column = 'synthetic_cache_status'
    else:
        status_column = 'cache_status'
    
    plt.figure(figsize=(10, 6))
    
    # Prepare data for boxplot
    data = []
    labels = []
    
    for status in cache_statuses:
        subset = client_df[client_df[status_column] == status]
        if len(subset) > 0:
            # Filter out outliers for better visualization
            rtts = subset['rtt_ms']
            q1, q3 = np.percentile(rtts, [25, 75])
            iqr = q3 - q1
            upper_bound = q3 + 1.5 * iqr
            filtered_rtts = rtts[rtts <= upper_bound]
            
            data.append(filtered_rtts)
            labels.append(f"{status} (n={len(filtered_rtts)})")
    
    if not data:
        print("Not enough data for latency vs cache status plot")
        return
    
    # Create boxplot
    plt.boxplot(data, labels=labels, patch_artist=True)
    
    # Add scatter points for individual RTTs (jittered for visibility)
    for i, (status, d) in enumerate(zip(cache_statuses, data)):
        # Add jitter
        x = np.random.normal(i+1, 0.05, size=len(d))
        plt.scatter(x, d, alpha=0.4, s=20)
    
    plt.title('End-to-End Latency vs Cache Status', fontsize=16)
    plt.ylabel('Latency (ms)', fontsize=14)
    plt.xlabel('Cache Status', fontsize=14)
    plt.grid(True, alpha=0.3, axis='y')
    
    # Add mean latency labels
    for i, d in enumerate(data):
        if len(d) > 0:
            plt.text(i+1, np.mean(d), f"Mean: {np.mean(d):.2f}ms", 
                    ha='center', va='bottom', fontweight='bold')
    
    plt.tight_layout()
    
    # Save plot
    plot_path = os.path.join(PLOTS_DIR, 'latency_vs_cache_status.png')
    plt.savefig(plot_path, dpi=300)
    print(f"Saved latency vs cache status plot to {plot_path}")
    plt.close()

def plot_mtu_vs_rtt(client_df):
    """Plot predicted MTU vs RTT"""
    if client_df is None:
        print("Cannot generate MTU vs RTT plot: missing data")
        return
    
    # Filter for entries with non-zero MTU values
    mtu_df = client_df[client_df['measured_mtu'] > 0].copy()
    
    if len(mtu_df) == 0:
        # Since we might not have actual MTU test data, create a synthetic plot
        # using request size as proxy for MTU
        mtu_df = client_df.copy()
        # Extract size from interest name if possible
        def extract_size(name):
            if isinstance(name, str) and 'size=' in name:
                try:
                    return int(name.split('size=')[1].split('/')[0])
                except:
                    pass
            return 0
        
        mtu_df['extracted_size'] = mtu_df['interest_name'].apply(extract_size)
        # Filter for entries with non-zero size
        mtu_df = mtu_df[mtu_df['extracted_size'] > 0]
        
        if len(mtu_df) == 0:
            print("Not enough data for MTU vs RTT plot")
            return
        
        # Use extracted size as proxy for MTU
        mtu_column = 'extracted_size'
        title = 'Request Size vs RTT (Synthetic MTU Plot)'
        xlabel = 'Request Size (bytes)'
    else:
        mtu_column = 'measured_mtu'
        title = 'Predicted MTU vs RTT'
        xlabel = 'MTU (bytes)'
    
    plt.figure(figsize=(12, 7))
    
    # Create scatter plot
    scatter = plt.scatter(mtu_df[mtu_column], mtu_df['rtt_ms'], 
                         alpha=0.7, c=mtu_df['rtt_ms'], cmap='viridis', 
                         s=50, edgecolors='k', linewidths=0.5)
    
    # Add colorbar
    cbar = plt.colorbar(scatter)
    cbar.set_label('RTT (ms)', fontsize=12)
    
    # Add trendline
    if len(mtu_df) >= 2:
        z = np.polyfit(mtu_df[mtu_column], mtu_df['rtt_ms'], 1)
        p = np.poly1d(z)
        plt.plot(sorted(mtu_df[mtu_column]), p(sorted(mtu_df[mtu_column])), 
                "r--", linewidth=2, label=f"Trend: y={z[0]:.6f}x+{z[1]:.2f}")
    
    # Format plot
    plt.title(title, fontsize=16)
    plt.xlabel(xlabel, fontsize=14)
    plt.ylabel('RTT (ms)', fontsize=14)
    plt.grid(True, alpha=0.3)
    if len(mtu_df) >= 2:
        plt.legend(fontsize=12)
    plt.tight_layout()
    
    # Add annotations for min and max values
    min_idx = mtu_df['rtt_ms'].idxmin()
    max_idx = mtu_df['rtt_ms'].idxmax()
    
    plt.annotate(f"Min RTT: {mtu_df.loc[min_idx, 'rtt_ms']:.2f}ms",
                xy=(mtu_df.loc[min_idx, mtu_column], mtu_df.loc[min_idx, 'rtt_ms']),
                xytext=(10, 20), textcoords='offset points',
                arrowprops=dict(arrowstyle='->', connectionstyle='arc3,rad=.2'))
    
    plt.annotate(f"Max RTT: {mtu_df.loc[max_idx, 'rtt_ms']:.2f}ms",
                xy=(mtu_df.loc[max_idx, mtu_column], mtu_df.loc[max_idx, 'rtt_ms']),
                xytext=(10, -20), textcoords='offset points',
                arrowprops=dict(arrowstyle='->', connectionstyle='arc3,rad=.2'))
    
    # Save plot
    plot_path = os.path.join(PLOTS_DIR, 'mtu_vs_rtt.png')
    plt.savefig(plot_path, dpi=300)
    print(f"Saved MTU vs RTT plot to {plot_path}")
    plt.close()

def main():
    print("Loading server metrics...")
    server_df = load_server_metrics()
    
    print("Loading client metrics...")
    client_df = load_client_metrics()
    
    print("Preprocessing metrics...")
    server_df, client_df = preprocess_metrics(server_df, client_df)
    
    print("Generating plots...")
    plot_cache_hit_rate_over_time(server_df)
    plot_latency_vs_cache_status(client_df, server_df)
    plot_mtu_vs_rtt(client_df)
    
    print("Done generating plots!")

if __name__ == "__main__":
    main()
