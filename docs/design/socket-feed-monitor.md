# SCUD Monitor Socket Feed Design

## Overview

This document describes the architecture for exposing SCUD's live monitor output via a ZMQ socket feed, enabling remote monitoring through web apps or arbitrary channels.

## Problem Statement

The current monitor architecture:
- Runs as a local TUI application
- Polls tmux via `capture-pane` every 500ms
- Reads session state from JSON files
- Cannot be accessed remotely

**Goal**: Create a ZMQ PUB socket feed that broadcasts monitor events, allowing:
- Web dashboards on remote machines
- Multiple concurrent observers
- Integration with arbitrary channels (Slack, Discord, etc.)
- Decoupled monitoring from the TUI

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Remote Worker                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────┐     ┌──────────────────────────────────┐  │
│  │ tmux Sessions    │     │ .scud/spawn/*.json               │  │
│  │ (agent windows)  │     │ (session metadata)               │  │
│  └────────┬─────────┘     └───────────────┬──────────────────┘  │
│           │                               │                      │
│           │ capture-pane                  │ file watch           │
│           ▼                               ▼                      │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │               FeedPublisher (Python)                       │  │
│  │                                                            │  │
│  │  • Watches session JSON files for changes                 │  │
│  │  • Captures tmux output periodically                      │  │
│  │  • Publishes structured events via ZMQ PUB socket         │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│                              │ ZMQ PUB (tcp://*:5555)           │
│                              ▼                                   │
└──────────────────────────────┼───────────────────────────────────┘
                               │
           ┌───────────────────┼───────────────────┐
           │                   │                   │
           ▼                   ▼                   ▼
    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
    │ Web App     │    │ CLI Client  │    │ Slack Bot   │
    │ Dashboard   │    │ (remote)    │    │ Integration │
    └─────────────┘    └─────────────┘    └─────────────┘
```

## Event Types

The feed publishes structured JSON events with topic-based filtering:

### Topic: `session`
Session-level events (session start/stop, configuration changes)

```json
{
  "type": "session",
  "event": "started",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "session_name": "scud-feature",
    "tag": "auth",
    "working_dir": "/home/user/project",
    "terminal": "tmux"
  }
}
```

### Topic: `agent`
Agent lifecycle events (spawn, status change, completion)

```json
{
  "type": "agent",
  "event": "status_changed",
  "timestamp": "2024-01-15T10:31:00Z",
  "data": {
    "task_id": "auth:1",
    "task_title": "Implement login flow",
    "window_name": "task-auth:1",
    "status": "running",
    "previous_status": "starting"
  }
}
```

### Topic: `output`
Terminal output lines (streamed as they appear)

```json
{
  "type": "output",
  "event": "lines",
  "timestamp": "2024-01-15T10:31:05Z",
  "data": {
    "task_id": "auth:1",
    "window_name": "task-auth:1",
    "lines": ["Processing request...", "Writing file..."],
    "sequence": 142
  }
}
```

### Topic: `stats`
Periodic statistics summary

```json
{
  "type": "stats",
  "event": "update",
  "timestamp": "2024-01-15T10:32:00Z",
  "data": {
    "session_name": "scud-feature",
    "total_agents": 5,
    "starting": 0,
    "running": 3,
    "completed": 1,
    "failed": 1
  }
}
```

### Topic: `task`
Task-level events from SCUD task graph

```json
{
  "type": "task",
  "event": "status_changed",
  "timestamp": "2024-01-15T10:33:00Z",
  "data": {
    "task_id": "auth:1",
    "status": "done",
    "previous_status": "in-progress"
  }
}
```

## Implementation

### FeedPublisher (Python)

Location: `scud-cli/scripts/feed_publisher.py`

Key responsibilities:
1. **File Watcher**: Use `watchdog` to monitor `.scud/spawn/*.json` for changes
2. **tmux Poller**: Periodically capture output from active agent windows
3. **Event Publisher**: Broadcast events via ZMQ PUB socket
4. **Diff Detection**: Only send output lines that are new (using sequence numbers)

### Subscriber Examples

#### Python CLI Client
```python
import zmq
import json

context = zmq.Context()
socket = context.socket(zmq.SUB)
socket.connect("tcp://remote-worker:5555")
socket.subscribe(b"agent")  # Only agent events
socket.subscribe(b"output")  # And output

while True:
    topic, payload = socket.recv_multipart()
    event = json.loads(payload)
    print(f"[{event['type']}] {event['event']}: {event['data']}")
```

#### WebSocket Bridge (for browser clients)
```python
import asyncio
import json
import websockets
import zmq
import zmq.asyncio

async def zmq_to_websocket(websocket):
    """Bridge ZMQ events to WebSocket client"""
    context = zmq.asyncio.Context()
    socket = context.socket(zmq.SUB)
    socket.connect("tcp://localhost:5555")
    socket.subscribe(b"")  # All events

    while True:
        topic, payload = await socket.recv_multipart()
        await websocket.send(payload.decode())
```

## Configuration

```toml
# .scud/feed.toml
[feed]
# ZMQ bind address
bind = "tcp://*:5555"

# Output polling interval (ms)
output_poll_interval = 500

# Stats broadcast interval (ms)
stats_interval = 5000

# Maximum output lines per message
max_output_lines = 50

# Enable output deduplication
dedupe_output = true
```

## Security Considerations

1. **Authentication**: ZMQ doesn't provide built-in auth. Options:
   - Use SSH tunneling for remote access
   - Implement CURVE encryption for ZMQ sockets
   - Use a reverse proxy with authentication

2. **Network Exposure**: By default, bind only to localhost
   - For remote access, use explicit configuration or SSH tunnel

3. **Data Sensitivity**: Monitor output may contain sensitive data
   - Consider filtering/redacting in production deployments

## Alternatives Considered

### WebSocket Server (direct)
- Pros: Browser-native, simpler for single web client
- Cons: More complex server, connection management overhead

### Redis Pub/Sub
- Pros: Persistence option, widely deployed
- Cons: Extra dependency, more ops overhead

### gRPC Streaming
- Pros: Type-safe, bidirectional
- Cons: More complex, overkill for this use case

### MQTT
- Pros: Lightweight, good for IoT/remote
- Cons: Requires broker, more infrastructure

**Decision**: ZMQ chosen for:
- Zero infrastructure (no broker)
- Simple pub/sub semantics
- Excellent Python support
- Low latency
- Topic-based filtering

## Future Extensions

1. **Bidirectional Control**: Add REQ/REP socket for sending commands
2. **Output Buffering**: Replay recent output for late-joining subscribers
3. **Compression**: Compress large output batches
4. **Metrics Export**: Prometheus endpoint for operational monitoring
