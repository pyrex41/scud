#!/usr/bin/env python3
"""
SCUD Monitor Feed Subscriber Examples

Demonstrates how to consume the monitor feed from various contexts:
1. CLI subscriber - print events to terminal
2. WebSocket bridge - forward events to browser clients
3. Filtered subscriber - only specific event types

Usage:
    # Basic CLI subscriber
    python feed_subscriber.py --connect tcp://localhost:5555

    # WebSocket bridge for browser clients
    python feed_subscriber.py --websocket --ws-port 8765

    # Filter to specific topics
    python feed_subscriber.py --topics output,agent
"""

import argparse
import asyncio
import json
import sys
from datetime import datetime
from typing import Optional

try:
    import zmq
    import zmq.asyncio
except ImportError:
    print("Error: pyzmq required. Install with: pip install pyzmq")
    sys.exit(1)

# Optional websockets for bridge mode
try:
    import websockets
    HAVE_WEBSOCKETS = True
except ImportError:
    HAVE_WEBSOCKETS = False


class Colors:
    """ANSI color codes for terminal output"""
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"

    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    MAGENTA = "\033[35m"
    CYAN = "\033[36m"
    WHITE = "\033[37m"

    @classmethod
    def colored(cls, text: str, color: str) -> str:
        return f"{color}{text}{cls.RESET}"


def format_event_cli(event: dict) -> str:
    """Format an event for CLI display"""
    event_type = event.get("type", "unknown")
    event_name = event.get("event", "unknown")
    timestamp = event.get("timestamp", "")
    data = event.get("data", {})

    # Parse timestamp for display
    try:
        dt = datetime.fromisoformat(timestamp.replace("Z", "+00:00"))
        time_str = dt.strftime("%H:%M:%S")
    except (ValueError, AttributeError):
        time_str = "??:??:??"

    # Color based on event type
    type_colors = {
        "session": Colors.CYAN,
        "agent": Colors.GREEN,
        "output": Colors.WHITE,
        "stats": Colors.MAGENTA,
        "task": Colors.YELLOW,
    }
    color = type_colors.get(event_type, Colors.WHITE)

    # Format based on event type
    if event_type == "output":
        task_id = data.get("task_id", "?")
        lines = data.get("lines", [])
        seq = data.get("sequence", 0)
        header = Colors.colored(f"[{time_str}] ", Colors.DIM)
        header += Colors.colored(f"{task_id}", Colors.BLUE)
        header += Colors.colored(f" (seq={seq})", Colors.DIM)
        output = [header]
        for line in lines[-10:]:  # Show last 10 lines
            output.append(f"  {line}")
        return "\n".join(output)

    elif event_type == "agent":
        task_id = data.get("task_id", "?")
        status = data.get("status", "?")
        prev = data.get("previous_status", "")

        status_colors = {
            "starting": Colors.YELLOW,
            "running": Colors.GREEN,
            "completed": Colors.CYAN,
            "failed": Colors.RED,
        }
        status_color = status_colors.get(status, Colors.WHITE)

        header = Colors.colored(f"[{time_str}] ", Colors.DIM)
        header += Colors.colored(f"AGENT ", color)
        header += Colors.colored(f"{event_name}: ", Colors.BOLD)
        header += Colors.colored(task_id, Colors.BLUE)
        header += " "
        if prev:
            header += Colors.colored(f"{prev}", Colors.DIM)
            header += " -> "
        header += Colors.colored(status, status_color)
        return header

    elif event_type == "stats":
        session = data.get("session_name", "?")
        total = data.get("total_agents", 0)
        running = data.get("running", 0)
        completed = data.get("completed", 0)
        failed = data.get("failed", 0)

        header = Colors.colored(f"[{time_str}] ", Colors.DIM)
        header += Colors.colored(f"STATS ", color)
        header += Colors.colored(f"{session}: ", Colors.BOLD)
        header += Colors.colored(f"{running}", Colors.GREEN)
        header += "/"
        header += Colors.colored(f"{completed}", Colors.CYAN)
        header += "/"
        header += Colors.colored(f"{failed}", Colors.RED)
        header += Colors.colored(f" ({total} total)", Colors.DIM)
        return header

    elif event_type == "session":
        session = data.get("session_name", data.get("bind_address", "?"))
        header = Colors.colored(f"[{time_str}] ", Colors.DIM)
        header += Colors.colored(f"SESSION ", color)
        header += Colors.colored(f"{event_name}: ", Colors.BOLD)
        header += session
        return header

    else:
        header = Colors.colored(f"[{time_str}] ", Colors.DIM)
        header += Colors.colored(f"{event_type.upper()} ", color)
        header += Colors.colored(f"{event_name}: ", Colors.BOLD)
        header += json.dumps(data, indent=2)
        return header


async def cli_subscriber(
    connect_address: str,
    topics: Optional[list[str]] = None,
    raw: bool = False
):
    """Subscribe to feed and print events to terminal"""
    context = zmq.asyncio.Context()
    socket = context.socket(zmq.SUB)
    socket.connect(connect_address)

    # Subscribe to topics
    if topics:
        for topic in topics:
            socket.subscribe(topic.encode())
            print(f"Subscribed to topic: {topic}")
    else:
        socket.subscribe(b"")  # All topics
        print("Subscribed to all topics")

    print(f"Connected to {connect_address}")
    print("-" * 60)

    try:
        while True:
            topic, payload = await socket.recv_multipart()
            event = json.loads(payload.decode())

            if raw:
                print(json.dumps(event, indent=2))
            else:
                print(format_event_cli(event))

    except KeyboardInterrupt:
        print("\nDisconnecting...")
    finally:
        socket.close()
        context.term()


async def websocket_bridge(
    zmq_address: str,
    ws_host: str = "localhost",
    ws_port: int = 8765,
    topics: Optional[list[str]] = None
):
    """Bridge ZMQ feed to WebSocket clients"""
    if not HAVE_WEBSOCKETS:
        print("Error: websockets required for bridge mode. Install with: pip install websockets")
        sys.exit(1)

    # Track connected WebSocket clients
    clients: set = set()

    # ZMQ setup
    zmq_context = zmq.asyncio.Context()
    zmq_socket = zmq_context.socket(zmq.SUB)
    zmq_socket.connect(zmq_address)

    if topics:
        for topic in topics:
            zmq_socket.subscribe(topic.encode())
    else:
        zmq_socket.subscribe(b"")

    async def ws_handler(websocket, path):
        """Handle WebSocket client connection"""
        clients.add(websocket)
        client_id = id(websocket)
        print(f"Client {client_id} connected from {websocket.remote_address}")

        try:
            # Keep connection alive, handle client messages
            async for message in websocket:
                # Could handle client -> server messages here
                # e.g., subscription filtering, commands
                pass
        except websockets.exceptions.ConnectionClosed:
            pass
        finally:
            clients.discard(websocket)
            print(f"Client {client_id} disconnected")

    async def zmq_forwarder():
        """Forward ZMQ messages to all WebSocket clients"""
        while True:
            try:
                topic, payload = await zmq_socket.recv_multipart()
                message = payload.decode()

                if clients:
                    # Broadcast to all connected clients
                    await asyncio.gather(
                        *[client.send(message) for client in clients],
                        return_exceptions=True
                    )
            except Exception as e:
                print(f"Error forwarding message: {e}")

    print(f"Starting WebSocket bridge")
    print(f"  ZMQ: {zmq_address}")
    print(f"  WebSocket: ws://{ws_host}:{ws_port}")

    # Start WebSocket server
    async with websockets.serve(ws_handler, ws_host, ws_port):
        # Run ZMQ forwarder
        await zmq_forwarder()


class EventAggregator:
    """Aggregates and summarizes events for dashboards"""

    def __init__(self):
        self.sessions: dict[str, dict] = {}
        self.agents: dict[str, dict] = {}  # task_id -> agent state
        self.recent_output: dict[str, list[str]] = {}  # task_id -> recent lines
        self.stats_history: list[dict] = []

    def process_event(self, event: dict) -> Optional[dict]:
        """Process an event and return aggregated state update if changed"""
        event_type = event.get("type")
        event_name = event.get("event")
        data = event.get("data", {})

        if event_type == "session":
            if event_name == "started":
                session_name = data.get("session_name")
                self.sessions[session_name] = data
            elif event_name == "stopped":
                session_name = data.get("session_name")
                self.sessions.pop(session_name, None)

        elif event_type == "agent":
            task_id = data.get("task_id")
            if event_name == "spawned":
                self.agents[task_id] = data
            elif event_name == "status_changed":
                if task_id in self.agents:
                    self.agents[task_id].update(data)
                else:
                    self.agents[task_id] = data

        elif event_type == "output":
            task_id = data.get("task_id")
            lines = data.get("lines", [])
            if task_id not in self.recent_output:
                self.recent_output[task_id] = []
            self.recent_output[task_id].extend(lines)
            # Keep last 100 lines
            self.recent_output[task_id] = self.recent_output[task_id][-100:]

        elif event_type == "stats":
            self.stats_history.append(data)
            # Keep last 100 stats snapshots
            self.stats_history = self.stats_history[-100:]

        # Return current state summary
        return self.get_state()

    def get_state(self) -> dict:
        """Get current aggregated state"""
        return {
            "sessions": list(self.sessions.values()),
            "agents": list(self.agents.values()),
            "stats": self.stats_history[-1] if self.stats_history else None,
        }


async def main():
    parser = argparse.ArgumentParser(
        description="SCUD Monitor Feed Subscriber"
    )
    parser.add_argument(
        "--connect", "-c",
        default="tcp://localhost:5555",
        help="ZMQ connect address (default: tcp://localhost:5555)"
    )
    parser.add_argument(
        "--topics", "-t",
        help="Comma-separated list of topics to subscribe to (default: all)"
    )
    parser.add_argument(
        "--raw", "-r",
        action="store_true",
        help="Output raw JSON instead of formatted"
    )
    parser.add_argument(
        "--websocket", "-w",
        action="store_true",
        help="Run as WebSocket bridge"
    )
    parser.add_argument(
        "--ws-host",
        default="localhost",
        help="WebSocket host (default: localhost)"
    )
    parser.add_argument(
        "--ws-port",
        type=int,
        default=8765,
        help="WebSocket port (default: 8765)"
    )

    args = parser.parse_args()

    topics = args.topics.split(",") if args.topics else None

    if args.websocket:
        await websocket_bridge(
            zmq_address=args.connect,
            ws_host=args.ws_host,
            ws_port=args.ws_port,
            topics=topics
        )
    else:
        await cli_subscriber(
            connect_address=args.connect,
            topics=topics,
            raw=args.raw
        )


if __name__ == "__main__":
    asyncio.run(main())
