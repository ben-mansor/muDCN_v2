#!/usr/bin/env python3
# NDN Test Client for μDCN Demo
# Generates NDN Interest packets and sends them over the specified interface

import argparse
import socket
import time
import random
import struct
import sys
import os
from datetime import datetime

# NDN TLV constants
NDN_INTEREST = 0x05
NDN_NAME = 0x07
NDN_NAME_COMPONENT = 0x08
NDN_NONCE = 0x0A
NDN_INTEREST_LIFETIME = 0x0C

def create_ndn_interest(name_components, nonce=None, lifetime_ms=4000):
    """
    Create an NDN Interest packet with the specified name components.
    
    Args:
        name_components: List of strings representing the name components
        nonce: Optional nonce value (generated randomly if None)
        lifetime_ms: Interest lifetime in milliseconds
        
    Returns:
        bytes: The complete NDN Interest packet
    """
    # Generate a random nonce if not provided
    if nonce is None:
        nonce = random.randint(0, 0xFFFFFFFF)
    
    # Encode name components
    encoded_components = b''
    for component in name_components:
        component_bytes = component.encode('utf-8')
        encoded_components += bytes([NDN_NAME_COMPONENT, len(component_bytes)]) + component_bytes
    
    # Encode name
    encoded_name = bytes([NDN_NAME, len(encoded_components)]) + encoded_components
    
    # Encode nonce (4 bytes)
    encoded_nonce = bytes([NDN_NONCE, 4]) + struct.pack("!I", nonce)
    
    # Encode interest lifetime (2 bytes)
    encoded_lifetime = bytes([NDN_INTEREST_LIFETIME, 2]) + struct.pack("!H", lifetime_ms)
    
    # Construct the Interest packet
    interest_value = encoded_name + encoded_nonce + encoded_lifetime
    
    # Add the Interest TLV header
    interest_packet = bytes([NDN_INTEREST, len(interest_value)]) + interest_value
    
    return interest_packet

def send_packet(packet, interface, destination_mac=None):
    """
    Send a raw packet over the specified interface.
    
    Args:
        packet: Bytes to send
        interface: Network interface to use
        destination_mac: Destination MAC address (broadcast if None)
    """
    # Create a raw socket
    try:
        s = socket.socket(socket.AF_PACKET, socket.SOCK_RAW)
        s.bind((interface, 0))
    except socket.error as e:
        print(f"Socket error: {e}")
        if e.errno == 1:
            print("Operation not permitted - Are you running as root?")
        sys.exit(1)
    
    # Set destination MAC to broadcast if not specified
    if destination_mac is None:
        destination_mac = b"\xff\xff\xff\xff\xff\xff"
    else:
        # Convert string format (e.g., "00:11:22:33:44:55") to bytes
        destination_mac = bytes.fromhex(destination_mac.replace(':', ''))
    
    # Get the source MAC address of the interface
    try:
        source_mac = bytes.fromhex(open(f"/sys/class/net/{interface}/address").read().strip().replace(':', ''))
    except:
        print(f"Could not get MAC address for interface {interface}")
        source_mac = b"\x00\x00\x00\x00\x00\x00"
    
    # Ethertype for NDN
    ethertype = b"\x86\x24"  # 0x8624
    
    # Construct the Ethernet frame
    ethernet_frame = destination_mac + source_mac + ethertype + packet
    
    # Send the packet
    s.send(ethernet_frame)
    s.close()

def generate_random_name():
    """Generate a random NDN name for testing"""
    prefixes = ["udcn", "ndn", "test", "demo", "content"]
    types = ["data", "video", "audio", "text", "image"]
    ids = [f"id{random.randint(1, 1000)}" for _ in range(3)]
    
    # Randomly select components
    name_components = [
        random.choice(prefixes),
        random.choice(types),
        random.choice(ids)
    ]
    
    # Add a timestamp component to ensure uniqueness
    name_components.append(f"ts{int(time.time())}")
    
    return name_components

def generate_sequential_name(index):
    """Generate a sequential NDN name for testing"""
    return ["udcn", "test", f"packet{index}", f"ts{int(time.time())}"]

def main():
    parser = argparse.ArgumentParser(description='NDN Interest Packet Generator')
    parser.add_argument('--interface', '-i', required=True, help='Network interface to use')
    parser.add_argument('--count', '-c', type=int, default=10, help='Number of packets to send')
    parser.add_argument('--interval', '-t', type=float, default=1.0, help='Time interval between packets (seconds)')
    parser.add_argument('--destination-mac', '-m', help='Destination MAC address (broadcast if not specified)')
    parser.add_argument('--random', '-r', action='store_true', help='Use random names instead of sequential')
    
    args = parser.parse_args()
    
    print(f"NDN Test Client - Sending {args.count} Interest packets on {args.interface}")
    print("-" * 60)
    
    for i in range(args.count):
        # Generate name
        if args.random:
            name_components = generate_random_name()
        else:
            name_components = generate_sequential_name(i+1)
        
        name_str = "/".join(name_components)
        
        # Create and send the Interest packet
        interest_packet = create_ndn_interest(name_components)
        send_packet(interest_packet, args.interface, args.destination_mac)
        
        print(f"[{datetime.now().strftime('%H:%M:%S')}] Sent Interest: {name_str}")
        
        # Wait before sending the next packet
        if i < args.count - 1:
            time.sleep(args.interval)
    
    print("-" * 60)
    print(f"Completed sending {args.count} Interest packets")

if __name__ == "__main__":
    main()
