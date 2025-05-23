#!/usr/bin/env python3
# μDCN Python Controller
# This script implements the control plane and ML integration

import argparse
import grpc
import threading
import time
import logging
import json
import os
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler
from concurrent import futures

# Add the current directory to the Python path
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

# Import ML components
sys.path.append(os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "ml_models"))
try:
    from ml_integration import MTUPredictorClient
except ImportError:
    print("Warning: ML integration module not found")

# Import generated gRPC classes
try:
    import udcn_pb2
    import udcn_pb2_grpc
except ImportError:
    print("Warning: gRPC bindings not found, trying to generate them")
    try:
        from generate_proto import generate_proto_files
        generate_proto_files()
        import udcn_pb2
        import udcn_pb2_grpc
    except Exception as e:
        print(f"Error generating proto files: {e}")
        sys.exit(1)

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("μDCN-Controller")

class SimpleHTTPHandler(BaseHTTPRequestHandler):
    """Simple HTTP handler for status and control API"""
    
    def _set_headers(self, content_type="application/json"):
        self.send_response(200)
        self.send_header('Content-type', content_type)
        self.end_headers()
    
    def do_GET(self):
        """Handle GET requests - provides status information"""
        if self.path == '/status':
            self._set_headers()
            status = {
                "status": "running",
                "uptime": time.time() - self.server.start_time,
                "stats": self.server.controller.get_stats()
            }
            self.wfile.write(json.dumps(status).encode())
        elif self.path == '/metrics':
            self._set_headers()
            metrics = self.server.controller.get_metrics()
            self.wfile.write(json.dumps(metrics).encode())
        elif self.path == '/predictions':
            self._set_headers()
            predictions = self.server.controller.get_recent_predictions()
            self.wfile.write(json.dumps(predictions).encode())
        else:
            self._set_headers("text/html")
            html = """
            <html>
            <head><title>μDCN Controller</title></head>
            <body>
                <h1>μDCN Controller</h1>
                <p>Available endpoints:</p>
                <ul>
                    <li><a href="/status">/status</a> - Controller status</li>
                    <li><a href="/metrics">/metrics</a> - Performance metrics</li>
                    <li><a href="/predictions">/predictions</a> - Recent MTU predictions</li>
                </ul>
            </body>
            </html>
            """
            self.wfile.write(html.encode())

class UDCNController:
    """Main controller for μDCN system"""
    
    def __init__(self, grpc_addr="localhost:50051"):
        self.grpc_addr = grpc_addr
        self.stats = {
            "interest_packets": 0,
            "data_packets": 0,
            "cache_hits": 0,
            "predicted_mtus": 0
        }
        self.recent_predictions = []
        self.metrics = {
            "rtt_ms": [],
            "packet_loss": [],
            "throughput_mbps": [],
            "predicted_mtu": []
        }
        
        # Initialize ML predictor
        try:
            self.mtu_predictor = MTUPredictorClient()
            logger.info("ML predictor initialized")
        except NameError:
            self.mtu_predictor = None
            logger.warning("ML predictor not available")
        
        # Initialize gRPC client
        self.channel = grpc.insecure_channel(self.grpc_addr)
        self.stub = udcn_pb2_grpc.UdcnControlStub(self.channel)
        logger.info(f"Connected to gRPC server at {self.grpc_addr}")
        
        # Start background tasks
        self.running = True
        self.monitor_thread = threading.Thread(target=self._monitor_task)
        self.monitor_thread.daemon = True
        self.monitor_thread.start()
    
    def _monitor_task(self):
        """Background task to monitor the system"""
        while self.running:
            try:
                # Collect metrics
                self._collect_metrics()
                
                # Make MTU predictions
                self._predict_mtu()
                
                # Sleep for a bit
                time.sleep(5)
            except Exception as e:
                logger.error(f"Error in monitor task: {e}")
                time.sleep(1)
    
    def _collect_metrics(self):
        """Collect performance metrics from the gRPC server"""
        try:
            # Example metrics collection - in a real system, these would come from the transport layer
            rtt = 20 + 10 * (0.5 - (time.time() % 60) / 60)  # Simulate RTT between 10-30ms
            loss = 0.01 + 0.02 * (0.5 - (time.time() % 45) / 45)  # Simulate loss between 0-3%
            throughput = 100 + 50 * (0.5 + (time.time() % 30) / 30)  # Simulate throughput between 75-125 Mbps
            
            # Store metrics
            self.metrics["rtt_ms"].append(rtt)
            self.metrics["packet_loss"].append(loss)
            self.metrics["throughput_mbps"].append(throughput)
            
            # Keep only the last 20 values
            self.metrics["rtt_ms"] = self.metrics["rtt_ms"][-20:]
            self.metrics["packet_loss"] = self.metrics["packet_loss"][-20:]
            self.metrics["throughput_mbps"] = self.metrics["throughput_mbps"][-20:]
            
            logger.debug(f"Collected metrics: RTT={rtt:.1f}ms, Loss={loss:.4f}, Throughput={throughput:.1f}Mbps")
        except Exception as e:
            logger.error(f"Error collecting metrics: {e}")
    
    def _predict_mtu(self):
        """Make MTU predictions based on current network conditions"""
        if not self.mtu_predictor:
            return
        
        try:
            # Get recent metrics
            rtt = self.metrics["rtt_ms"][-1] if self.metrics["rtt_ms"] else 20
            loss = self.metrics["packet_loss"][-1] if self.metrics["packet_loss"] else 0.01
            throughput = self.metrics["throughput_mbps"][-1] if self.metrics["throughput_mbps"] else 100
            
            # Make prediction using ML model
            request = udcn_pb2.MtuPredictionRequest(
                rtt_ms=rtt,
                packet_loss_rate=loss,
                throughput_mbps=throughput
            )
            
            # Call gRPC method
            response = self.stub.PredictMtu(request)
            
            if response.success:
                mtu = response.predicted_mtu
                self.metrics["predicted_mtu"].append(mtu)
                self.metrics["predicted_mtu"] = self.metrics["predicted_mtu"][-20:]
                
                # Store prediction with timestamp
                prediction = {
                    "timestamp": time.time(),
                    "rtt_ms": rtt,
                    "packet_loss_rate": loss,
                    "throughput_mbps": throughput,
                    "predicted_mtu": mtu,
                    "confidence": response.confidence
                }
                self.recent_predictions.append(prediction)
                self.recent_predictions = self.recent_predictions[-10:]
                
                # Update stats
                self.stats["predicted_mtus"] += 1
                
                logger.info(f"MTU prediction: {mtu} bytes (RTT={rtt:.1f}ms, Loss={loss:.4f}, Throughput={throughput:.1f}Mbps)")
            else:
                logger.warning(f"MTU prediction failed: {response.error_message}")
        except Exception as e:
            logger.error(f"Error making MTU prediction: {e}")
    
    def get_stats(self):
        """Get current stats"""
        return self.stats
    
    def get_metrics(self):
        """Get collected metrics"""
        return self.metrics
    
    def get_recent_predictions(self):
        """Get recent MTU predictions"""
        return self.recent_predictions
    
    def shutdown(self):
        """Shutdown the controller"""
        self.running = False
        if self.monitor_thread.is_alive():
            self.monitor_thread.join(timeout=1)
        self.channel.close()
        logger.info("Controller shutdown complete")

def run_web_server(controller, port):
    """Run the web server for the controller API"""
    server = HTTPServer(('0.0.0.0', port), SimpleHTTPHandler)
    server.controller = controller
    server.start_time = time.time()
    logger.info(f"Web server started on port {port}")
    server.serve_forever()

def main():
    parser = argparse.ArgumentParser(description='μDCN Python Controller')
    parser.add_argument('--grpc-port', type=int, default=50051, help='gRPC server port')
    parser.add_argument('--web-port', type=int, default=8000, help='Web server port')
    parser.add_argument('--log-level', default='INFO', help='Logging level')
    
    args = parser.parse_args()
    
    # Set logging level
    logging.getLogger().setLevel(getattr(logging, args.log_level.upper()))
    
    # Print banner
    print("=" * 60)
    print("μDCN Python Controller")
    print("=" * 60)
    
    # Create and start controller
    grpc_addr = f"localhost:{args.grpc_port}"
    controller = UDCNController(grpc_addr=grpc_addr)
    
    try:
        # Start web server
        web_thread = threading.Thread(target=run_web_server, args=(controller, args.web_port))
        web_thread.daemon = True
        web_thread.start()
        
        # Keep the main thread alive
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nShutting down...")
    finally:
        controller.shutdown()

if __name__ == "__main__":
    main()
