#!/usr/bin/env python3
import socket
import struct
import time
import argparse
import sys

# NDN TLV types
TLV_INTEREST = 0x05
TLV_DATA = 0x06
TLV_NAME = 0x07
TLV_COMPONENT = 0x08
TLV_NONCE = 0x0A

def create_ndn_interest(name):
    """Create an NDN Interest packet with the given name"""
    # Encode name components
    encoded_name = b''
    components = name.strip('/').split('/')
    
    for component in components:
        comp_bytes = component.encode('utf-8')
        # TLV: type (TLV_COMPONENT), length, value
        encoded_name += struct.pack('!BB', TLV_COMPONENT, len(comp_bytes))
        encoded_name += comp_bytes
    
    # Encode name TLV
    name_tlv = struct.pack('!BB', TLV_NAME, len(encoded_name)) + encoded_name
    
    # Add nonce (4 bytes)
    nonce = struct.pack('!BI', TLV_NONCE, 4) + struct.pack('!I', 12345)
    
    # Create Interest packet
    interest_content = name_tlv + nonce
    interest_packet = struct.pack('!BB', TLV_INTEREST, len(interest_content)) + interest_content
    
    return interest_packet

def send_packet(interface, dest_ip, dest_port, packet):
    """Send a raw UDP packet on the specified interface"""
    # Create UDP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_BINDTODEVICE, interface.encode('utf-8'))
    
    try:
        sock.sendto(packet, (dest_ip, dest_port))
        return True
    except Exception as e:
        print(f"Error sending packet: {e}", file=sys.stderr)
        return False
    finally:
        sock.close()

def main():
    parser = argparse.ArgumentParser(description='Send NDN Interest packets')
    parser.add_argument('-i', '--interface', default='veth1', help='Interface to send packet from')
    parser.add_argument('-d', '--dest-ip', default='192.168.100.1', help='Destination IP address')
    parser.add_argument('-p', '--port', type=int, default=6363, help='Destination port')
    parser.add_argument('-n', '--name', required=True, help='NDN name to request')
    parser.add_argument('-c', '--count', type=int, default=1, help='Number of packets to send')
    parser.add_argument('--delay', type=float, default=0.5, help='Delay between packets in seconds')
    
    args = parser.parse_args()
    
    # Create NDN Interest packet
    packet = create_ndn_interest(args.name)
    
    print(f"Sending {args.count} Interest packet(s) for name: {args.name}")
    
    for i in range(args.count):
        success = send_packet(args.interface, args.dest_ip, args.port, packet)
        if success:
            print(f"Packet {i+1}/{args.count} sent successfully")
        else:
            print(f"Failed to send packet {i+1}/{args.count}")
        
        if i < args.count - 1:
            time.sleep(args.delay)
    
    print("Done sending packets")

if __name__ == "__main__":
    main()
