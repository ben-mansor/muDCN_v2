#!/usr/bin/env python3
"""
μDCN Benchmark Publication Plot Generator

This script generates publication-ready, professionally formatted plots for academic use,
visualizing the key performance metrics from the μDCN benchmark tests.
"""

import os
import glob
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.dates as mdates
from datetime import datetime
from matplotlib.ticker import PercentFormatter
import matplotlib as mpl

# Publication-quality settings
plt.rcParams.update({
    'font.family': 'serif',
    'font.size': 11,
    'axes.labelsize': 12,
    'axes.titlesize': 14,
    'xtick.labelsize': 10,
    'ytick.labelsize': 10,
    'legend.fontsize': 10,
    'figure.titlesize': 16,
    'figure.figsize': (7, 5),
    'savefig.dpi': 300,
    'savefig.bbox': 'tight',
    'savefig.pad_inches': 0.05,
})

# Configuration
METRICS_DIR = "/app/metrics"
PLOTS_DIR = "/app/results/plots"
PLOT_FORMATS = ['png', 'pdf']

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
    # Find all client metrics files
    client_files = glob.glob(os.path.join(METRICS_DIR, "*_metrics.csv"))
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
    # Convert timestamp to datetime and seconds since start
    if server_df is not None:
        server_df['datetime'] = pd.to_datetime(server_df['timestamp'], unit='ms')
        min_timestamp = server_df['timestamp'].min()
        server_df['seconds'] = (server_df['timestamp'] - min_timestamp) / 1000
        
    if client_df is not None:
        client_df['datetime'] = pd.to_datetime(client_df['timestamp'], unit='ms')
        # Calculate seconds relative to server start if possible
        if server_df is not None:
            min_timestamp = server_df['timestamp'].min()
        else:
            min_timestamp = client_df['timestamp'].min()
        client_df['seconds'] = (client_df['timestamp'] - min_timestamp) / 1000
        
        # Extract benchmark type and cache status from interest_name
        client_df['benchmark_type'] = client_df['interest_name'].apply(
            lambda x: x.split('/')[2] if isinstance(x, str) and len(x.split('/')) > 2 else 'unknown'
        )
        
        def infer_cache_status(name):
            if not isinstance(name, str):
                return 'unknown'
            if '/cache/cold/' in name:
                return 'miss'
            elif '/cache/warm/' in name:
                return 'hit'
            else:
                # Infer from success and data_size
                return 'unknown'
        
        client_df['cache_status'] = client_df['interest_name'].apply(infer_cache_status)
    
    return server_df, client_df

def plot_cache_hit_rate_over_time(server_df, output_dir=PLOTS_DIR):
    """Generate publication-ready cache hit rate over time plot"""
    if server_df is None or 'cache_hit_ratio' not in server_df.columns:
        print("Cannot generate cache hit rate plot: missing data")
        return
    
    # Create figure with specific size for publication
    fig, ax = plt.subplots(figsize=(8, 5))
    
    # Plot cache hit ratio as percentage
    ax.plot(server_df['seconds'], server_df['cache_hit_ratio'] * 100, 
            'b-', linewidth=2, label='Cache Hit Rate')
    
    # Add light blue fill below the line
    ax.fill_between(server_df['seconds'], 0, server_df['cache_hit_ratio'] * 100, 
                   alpha=0.2, color='blue')
    
    # Determine warm-up period (when hit rate starts consistently rising)
    # Simple approach: first third of the timeline is considered warm-up
    warmup_end = server_df['seconds'].max() / 3
    
    # Highlight warm-up period with light background
    ax.axvspan(0, warmup_end, alpha=0.2, color='gray', label='Warm-up Period')
    
    # Add a horizontal line at 50% hit rate
    ax.axhline(y=50, color='darkgray', linestyle='--', alpha=0.7)
    
    # Annotate stable region
    stable_start = warmup_end
    ax.text(stable_start + (server_df['seconds'].max() - stable_start)/2, 10, 
            'Stable Operation', ha='center', bbox=dict(facecolor='white', alpha=0.7))
    
    # Format plot
    ax.set_title('μDCN Cache Hit Rate Over Time', fontweight='bold')
    ax.set_xlabel('Time (seconds)', fontweight='bold')
    ax.set_ylabel('Hit Rate (%)', fontweight='bold')
    ax.set_ylim(0, 100)
    ax.set_xlim(0, server_df['seconds'].max())
    
    # Add grid for readability
    ax.grid(True, alpha=0.3)
    
    # Format y-axis as percentage
    ax.yaxis.set_major_formatter(PercentFormatter())
    
    # Add legend with shadow for better visibility on light/dark backgrounds
    legend = ax.legend(loc='lower right', frameon=True, framealpha=0.9,
                      shadow=True, fancybox=True)
    
    # Add explanatory text about cache behavior
    avg_hit_rate = server_df['cache_hit_ratio'].mean() * 100
    ax.text(0.02, 0.02, f'Average Hit Rate: {avg_hit_rate:.2f}%',
            transform=ax.transAxes, bbox=dict(facecolor='white', alpha=0.7))
    
    # Use tight layout to maximize plot area
    plt.tight_layout()
    
    # Save in multiple formats
    for fmt in PLOT_FORMATS:
        output_path = os.path.join(output_dir, f'cache_hit_rate_over_time.{fmt}')
        plt.savefig(output_path, format=fmt)
        print(f"Saved cache hit rate plot to {output_path}")
    
    plt.close()

def plot_latency_vs_cache_status(client_df, server_df, output_dir=PLOTS_DIR):
    """Generate publication-ready latency vs cache status boxplot"""
    if client_df is None:
        print("Cannot generate latency vs cache status plot: missing data")
        return
    
    # Create figure with specific size for publication
    fig, ax = plt.subplots(figsize=(8, 6))
    
    # Determine cache status - if we don't have explicit cache status in data,
    # we'll infer it based on server-side hit ratio trends
    if 'cache_status' in client_df.columns and any(client_df['cache_status'].isin(['hit', 'miss'])):
        # We have explicit cache status data
        status_column = 'cache_status'
        statuses = ['hit', 'miss']
    else:
        # Synthetic approach: use server hit ratio to infer cache status
        # by grouping RTTs into high-hit-rate periods and low-hit-rate periods
        if server_df is not None and 'cache_hit_ratio' in server_df.columns:
            # Find timestamp ranges with high hit ratio
            high_hit_threshold = 0.65  # 65% hit rate as threshold
            high_hit_periods = server_df[server_df['cache_hit_ratio'] >= high_hit_threshold]['timestamp']
            low_hit_periods = server_df[server_df['cache_hit_ratio'] < high_hit_threshold]['timestamp']
            
            # Classify client requests based on timestamp
            def classify_period(timestamp):
                if timestamp in high_hit_periods.values:
                    return 'Likely Hit'
                elif timestamp in low_hit_periods.values:
                    return 'Likely Miss'
                else:
                    return 'Unknown'
            
            client_df['inferred_status'] = client_df['timestamp'].apply(classify_period)
            status_column = 'inferred_status'
            statuses = ['Likely Hit', 'Likely Miss']
        else:
            # If we can't infer from server data, create a generic analysis
            # Group by success status instead
            client_df['inferred_status'] = client_df['success'].apply(lambda x: 'Success' if x == 1 else 'Failure')
            status_column = 'inferred_status'
            statuses = ['Success', 'Failure']
    
    # Prepare data for boxplot
    data = []
    labels = []
    colors = ['#3498db', '#e74c3c']
    
    for status, color in zip(statuses, colors):
        subset = client_df[client_df[status_column] == status]
        if len(subset) > 0:
            # Filter out extreme outliers for better visualization
            rtts = subset['rtt_ms']
            q1, q3 = np.percentile(rtts, [25, 75])
            iqr = q3 - q1
            upper_bound = q3 + 3 * iqr  # less strict filtering for academic plots
            filtered_rtts = rtts[rtts <= upper_bound]
            
            if len(filtered_rtts) > 0:
                data.append(filtered_rtts)
                mean_rtt = filtered_rtts.mean()
                count = len(filtered_rtts)
                labels.append(f"{status}\n(n={count}, avg={mean_rtt:.2f}ms)")
    
    if not data:
        print("Not enough data for latency vs cache status plot")
        return
    
    # Create boxplot with custom appearance
    boxprops = dict(linestyle='-', linewidth=1.5)
    whiskerprops = dict(linestyle='-', linewidth=1.5)
    medianprops = dict(linestyle='-', linewidth=2, color='black')
    
    # Create boxplot
    box = ax.boxplot(data, labels=labels, patch_artist=True, 
               boxprops=boxprops, whiskerprops=whiskerprops, medianprops=medianprops,
               showfliers=False)  # Hide outliers for cleaner appearance
    
    # Customize box colors
    for patch, color in zip(box['boxes'], colors):
        patch.set_facecolor(color)
        patch.set_alpha(0.6)
    
    # Add scatter points for individual RTTs (jittered for visibility)
    for i, (d, color) in enumerate(zip(data, colors)):
        # Add jitter
        x = np.random.normal(i+1, 0.08, size=len(d))
        scatter = ax.scatter(x, d, alpha=0.4, s=15, c=color, edgecolors='none')
    
    # Format plot
    ax.set_title('End-to-End Latency by Cache Status', fontweight='bold')
    ax.set_ylabel('Round-Trip Time (ms)', fontweight='bold')
    ax.set_xlabel('Cache Status', fontweight='bold')
    ax.grid(True, axis='y', alpha=0.3, linestyle='--')
    
    # Add statistical annotations
    if len(data) >= 2:
        # Calculate percentage difference between means
        mean1 = np.mean(data[0])
        mean2 = np.mean(data[1])
        percent_diff = abs(mean1 - mean2) / max(mean1, mean2) * 100
        
        # Add text with statistical significance
        if mean1 < mean2:
            comparison = f"{labels[0].split()[0]} is {percent_diff:.1f}% faster"
        else:
            comparison = f"{labels[1].split()[0]} is {percent_diff:.1f}% faster"
            
        ax.text(0.5, 0.01, comparison,
                ha='center', va='bottom', transform=ax.transAxes,
                bbox=dict(facecolor='white', alpha=0.8, boxstyle='round,pad=0.5'))
    
    # Use tight layout to maximize plot area
    plt.tight_layout()
    
    # Save in multiple formats
    for fmt in PLOT_FORMATS:
        output_path = os.path.join(output_dir, f'latency_vs_cache_status.{fmt}')
        plt.savefig(output_path, format=fmt)
        print(f"Saved latency vs cache status plot to {output_path}")
    
    plt.close()

def plot_mtu_vs_rtt(client_df, output_dir=PLOTS_DIR):
    """Generate publication-ready MTU prediction vs RTT plot"""
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
        title = 'Request Size vs. Round-Trip Time with MTU Prediction'
        xlabel = 'Request Size (bytes)'
    else:
        mtu_column = 'measured_mtu'
        title = 'Predicted MTU vs. Round-Trip Time'
        xlabel = 'MTU (bytes)'
    
    # Create figure with specific size for publication
    fig, ax = plt.subplots(figsize=(8, 6))
    
    # Create custom colormap for network condition visualization
    # We'll color by RTT - blue is low, red is high
    norm = plt.Normalize(mtu_df['rtt_ms'].min(), mtu_df['rtt_ms'].max())
    cmap = plt.cm.coolwarm
    
    # Create scatter plot
    scatter = ax.scatter(mtu_df[mtu_column], mtu_df['rtt_ms'], 
                       alpha=0.7, c=mtu_df['rtt_ms'], cmap=cmap, norm=norm,
                       s=50, edgecolors='k', linewidths=0.5)
    
    # Add colorbar
    cbar = fig.colorbar(scatter, ax=ax, pad=0.02)
    cbar.set_label('Round-Trip Time (ms)', fontweight='bold')
    
    # Add trendline
    if len(mtu_df) >= 2:
        z = np.polyfit(mtu_df[mtu_column], mtu_df['rtt_ms'], 1)
        p = np.poly1d(z)
        x_pred = np.linspace(mtu_df[mtu_column].min(), mtu_df[mtu_column].max(), 100)
        ax.plot(x_pred, p(x_pred), "k--", linewidth=2, 
                label=f"Trend: y = {z[0]:.4f}x + {z[1]:.2f}")
        
        # Calculate and display correlation coefficient
        correlation = np.corrcoef(mtu_df[mtu_column], mtu_df['rtt_ms'])[0, 1]
        ax.text(0.05, 0.95, f"Correlation: {correlation:.3f}", 
                transform=ax.transAxes, fontweight='bold',
                bbox=dict(facecolor='white', alpha=0.8))
    
    # Label key prediction points
    if len(mtu_df) > 0:
        # Label min and max RTT points
        min_idx = mtu_df['rtt_ms'].idxmin()
        max_idx = mtu_df['rtt_ms'].idxmax()
        
        # For each labeled point, add network context
        for idx, label in [(min_idx, 'Min RTT'), (max_idx, 'Max RTT')]:
            rtt = mtu_df.loc[idx, 'rtt_ms']
            mtu = mtu_df.loc[idx, mtu_column]
            
            # Get network conditions if available
            net_context = f"{label}: {rtt:.2f}ms @ {mtu}B"
            
            # Add annotation with arrow
            ax.annotate(net_context,
                      xy=(mtu, rtt),
                      xytext=(20 if idx == min_idx else -20, 20 if idx == min_idx else -20), 
                      textcoords="offset points",
                      arrowprops=dict(arrowstyle="->", connectionstyle="arc3,rad=.2", 
                                      color='black'),
                      bbox=dict(boxstyle="round,pad=0.3", fc="white", alpha=0.8))
    
    # Format plot
    ax.set_title(title, fontweight='bold')
    ax.set_xlabel(xlabel, fontweight='bold')
    ax.set_ylabel('Round-Trip Time (ms)', fontweight='bold')
    ax.grid(True, alpha=0.3, linestyle='--')
    
    # Add MTU prediction explanation
    if 'benchmark_type' in mtu_df.columns and 'mtu' in ''.join(mtu_df['benchmark_type'].unique()).lower():
        ax.text(0.5, 0.02, 
                "MTU predictions adapt to network conditions to optimize performance",
                ha='center', transform=ax.transAxes,
                bbox=dict(facecolor='white', alpha=0.8, boxstyle='round,pad=0.5'))
    
    if len(mtu_df) >= 2:
        ax.legend(loc='upper right')
    
    # Use tight layout to maximize plot area
    plt.tight_layout()
    
    # Save in multiple formats
    for fmt in PLOT_FORMATS:
        output_path = os.path.join(output_dir, f'mtu_vs_rtt.{fmt}')
        plt.savefig(output_path, format=fmt)
        print(f"Saved MTU vs RTT plot to {output_path}")
    
    plt.close()

# Main execution
def main():
    print("Loading server metrics...")
    server_df = load_server_metrics()
    
    print("Loading client metrics...")
    client_df = load_client_metrics()
    
    print("Preprocessing metrics...")
    server_df, client_df = preprocess_metrics(server_df, client_df)
    
    print("Generating publication-ready plots...")
    plot_cache_hit_rate_over_time(server_df)
    plot_latency_vs_cache_status(client_df, server_df)
    plot_mtu_vs_rtt(client_df)
    
    print("Done generating publication-ready plots!")

if __name__ == "__main__":
    main()
