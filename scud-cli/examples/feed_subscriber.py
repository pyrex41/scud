#!/usr/bin/env python3
"""
SCUD Monitor Socket Feed Subscriber

Example subscriber for the SCUD monitor ZMQ PUB/SUB feed.
Demonstrates how to connect to the feed and process real-time updates.

Usage:
    # Start monitor with feed enabled
    scud monitor --session my-session --feed tcp://*:5555

    # Run this subscriber
    python examples/feed_subscriber.py

    # Or connect to a remote monitor
    python examples/feed_subscriber.py tcp://192.168.1.100:5555

Topics:
    - session: Full session snapshots (every 5s)
    - agent: Individual agent status changes
    - output: Live terminal output lines
    - wave: Wave/task progress updates
    - stats: Aggregate statistics
    - heartbeat: Feed liveness (every 5s)

Requirements:
    pip install pyzmq
"""

import json
import sys
from datetime import datetime

try:
    import zmq
except ImportError:
    print("Error: pyzmq not installed. Run: pip install pyzmq")
    sys.exit(1)


def parse_message(raw: bytes) -> tuple[str, dict]:
    """Parse a ZMQ message into topic and payload."""
    text = raw.decode("utf-8")
    # Format: "topic json_payload"
    space_idx = text.find(" ")
    if space_idx == -1:
        return text, {}
    topic = text[:space_idx]
    payload = text[space_idx + 1 :]
    try:
        data = json.loads(payload)
    except json.JSONDecodeError:
        data = {"raw": payload}
    return topic, data


def format_status(status: str) -> str:
    """Format status with colors (ANSI escape codes)."""
    colors = {
        "starting": "\033[33m",  # yellow
        "running": "\033[32m",  # green
        "completed": "\033[34m",  # blue
        "failed": "\033[31m",  # red
    }
    reset = "\033[0m"
    color = colors.get(status.lower(), "")
    return f"{color}{status}{reset}"


def handle_session(data: dict):
    """Handle session snapshot."""
    print(f"\n{'='*60}")
    print(f"SESSION: {data.get('session_name', 'unknown')}")
    print(f"Tag: {data.get('tag', '-')}")
    print(f"Agents: {len(data.get('agents', []))}")

    stats = data.get("stats", {})
    print(
        f"Status: {stats.get('starting', 0)} starting, "
        f"{stats.get('running', 0)} running, "
        f"{stats.get('completed', 0)} completed, "
        f"{stats.get('failed', 0)} failed"
    )

    for agent in data.get("agents", []):
        status = format_status(agent.get("status", "unknown"))
        print(f"  - [{status}] {agent.get('task_id', '?')}: {agent.get('task_title', '-')}")
    print(f"{'='*60}\n")


def handle_agent_update(data: dict):
    """Handle agent status change."""
    task_id = data.get("task_id", "?")
    status = format_status(data.get("status", "unknown"))
    prev = data.get("previous_status", "-")
    print(f"[AGENT] {task_id}: {prev} -> {status}")


def handle_output(data: dict):
    """Handle live output."""
    task_id = data.get("task_id", "?")
    lines = data.get("lines", [])
    if lines:
        # Only show last few lines to avoid spam
        print(f"[OUTPUT:{task_id}] ({len(lines)} lines)")
        for line in lines[-3:]:
            print(f"  | {line}")


def handle_wave(data: dict):
    """Handle wave update."""
    ready = data.get("ready_count", 0)
    running = data.get("running_count", 0)
    done = data.get("done_count", 0)
    blocked = data.get("blocked_count", 0)

    print(
        f"[WAVE] Ready: {ready}, Running: {running}, "
        f"Done: {done}, Blocked: {blocked}"
    )


def handle_stats(data: dict):
    """Handle stats update."""
    print(
        f"[STATS] {data.get('session_name', '?')}: "
        f"{data.get('running', 0)}/{data.get('total_agents', 0)} running, "
        f"{data.get('completed', 0)} done"
    )


def handle_heartbeat(data: dict):
    """Handle heartbeat."""
    uptime = data.get("uptime_secs", 0)
    msgs = data.get("message_count", 0)
    print(f"[HEARTBEAT] Uptime: {uptime}s, Messages: {msgs}")


HANDLERS = {
    "session": handle_session,
    "agent": handle_agent_update,
    "output": handle_output,
    "wave": handle_wave,
    "stats": handle_stats,
    "heartbeat": handle_heartbeat,
}


def main():
    endpoint = sys.argv[1] if len(sys.argv) > 1 else "tcp://localhost:5555"

    print(f"Connecting to SCUD feed at {endpoint}...")

    context = zmq.Context()
    socket = context.socket(zmq.SUB)

    # Subscribe to all topics (empty string = all)
    socket.setsockopt_string(zmq.SUBSCRIBE, "")

    # Optional: subscribe to specific topics only
    # socket.setsockopt_string(zmq.SUBSCRIBE, "agent")
    # socket.setsockopt_string(zmq.SUBSCRIBE, "output")

    socket.connect(endpoint)
    print(f"Connected! Waiting for messages...\n")

    try:
        while True:
            raw = socket.recv()
            topic, data = parse_message(raw)

            handler = HANDLERS.get(topic)
            if handler:
                handler(data)
            else:
                print(f"[{topic}] {data}")

    except KeyboardInterrupt:
        print("\nDisconnecting...")
    finally:
        socket.close()
        context.term()


if __name__ == "__main__":
    main()
