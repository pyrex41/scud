#!/usr/bin/env python3
"""
SCUD Monitor Feed Publisher

Publishes live monitor events via ZMQ PUB socket for remote monitoring.
Watches session files and tmux output, broadcasting structured events.

Usage:
    python feed_publisher.py [--bind tcp://*:5555] [--session SESSION_NAME]

Events are published as multipart messages: [topic, json_payload]
Topics: session, agent, output, stats, task
"""

import argparse
import asyncio
import hashlib
import json
import os
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

try:
    import zmq
    import zmq.asyncio
except ImportError:
    print("Error: pyzmq required. Install with: pip install pyzmq")
    sys.exit(1)

# Optional: watchdog for efficient file watching
try:
    from watchdog.events import FileSystemEventHandler
    from watchdog.observers import Observer
    HAVE_WATCHDOG = True
except ImportError:
    HAVE_WATCHDOG = False


@dataclass
class FeedEvent:
    """Structured event for the feed"""
    type: str
    event: str
    timestamp: str
    data: dict[str, Any]

    def to_json(self) -> bytes:
        return json.dumps(asdict(self), ensure_ascii=False).encode('utf-8')


@dataclass
class AgentOutputState:
    """Tracks output state for an agent to enable diffing"""
    task_id: str
    window_name: str
    last_hash: str = ""
    sequence: int = 0
    last_lines: list[str] = field(default_factory=list)


class SessionWatcher:
    """Watches session JSON files for changes"""

    def __init__(self, scud_dir: Path, callback):
        self.scud_dir = scud_dir
        self.spawn_dir = scud_dir / "spawn"
        self.callback = callback
        self._last_states: dict[str, str] = {}  # session_name -> json hash

    async def check_sessions(self) -> list[dict]:
        """Check for session file changes, return list of changed sessions"""
        if not self.spawn_dir.exists():
            return []

        changed = []
        current_files = set()

        for path in self.spawn_dir.glob("*.json"):
            session_name = path.stem
            current_files.add(session_name)

            try:
                content = path.read_text()
                content_hash = hashlib.md5(content.encode()).hexdigest()

                if session_name not in self._last_states:
                    # New session
                    self._last_states[session_name] = content_hash
                    session_data = json.loads(content)
                    changed.append(("new", session_name, session_data))
                elif self._last_states[session_name] != content_hash:
                    # Changed session
                    self._last_states[session_name] = content_hash
                    session_data = json.loads(content)
                    changed.append(("changed", session_name, session_data))
            except (json.JSONDecodeError, IOError) as e:
                print(f"Warning: Error reading {path}: {e}")

        # Check for removed sessions
        for session_name in list(self._last_states.keys()):
            if session_name not in current_files:
                del self._last_states[session_name]
                changed.append(("removed", session_name, None))

        return changed


class TmuxOutputCapture:
    """Captures output from tmux windows"""

    def __init__(self, session_name: str, max_lines: int = 100):
        self.session_name = session_name
        self.max_lines = max_lines
        self._agent_states: dict[str, AgentOutputState] = {}

    def get_tmux_windows(self) -> list[str]:
        """Get list of window names in the tmux session"""
        try:
            result = subprocess.run(
                ["tmux", "list-windows", "-t", self.session_name, "-F", "#{window_name}"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0:
                return [w.strip() for w in result.stdout.strip().split('\n') if w.strip()]
        except (subprocess.TimeoutExpired, FileNotFoundError):
            pass
        return []

    def capture_window(self, window_name: str) -> Optional[list[str]]:
        """Capture output from a tmux window"""
        try:
            result = subprocess.run(
                ["tmux", "capture-pane", "-t", f"{self.session_name}:{window_name}",
                 "-p", "-S", f"-{self.max_lines}"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0:
                lines = result.stdout.split('\n')
                # Strip trailing empty lines
                while lines and not lines[-1].strip():
                    lines.pop()
                return lines
        except (subprocess.TimeoutExpired, FileNotFoundError):
            pass
        return None

    def get_output_diff(self, task_id: str, window_name: str) -> Optional[tuple[list[str], int]]:
        """
        Get new output lines for an agent window.
        Returns (new_lines, sequence_number) or None if no changes.
        """
        lines = self.capture_window(window_name)
        if lines is None:
            return None

        # Get or create state for this agent
        if task_id not in self._agent_states:
            self._agent_states[task_id] = AgentOutputState(
                task_id=task_id,
                window_name=window_name
            )

        state = self._agent_states[task_id]

        # Hash current output
        content_hash = hashlib.md5('\n'.join(lines).encode()).hexdigest()

        if content_hash == state.last_hash:
            return None  # No change

        # Find new lines by comparing with last captured
        new_lines = []
        if state.last_lines:
            # Simple diff: find where current output diverges from last
            # This handles both new lines appended and scrollback
            last_set = set(state.last_lines[-20:])  # Use last 20 lines for matching
            for i, line in enumerate(lines):
                if line not in last_set:
                    new_lines = lines[i:]
                    break
            if not new_lines and len(lines) > len(state.last_lines):
                new_lines = lines[len(state.last_lines):]
        else:
            new_lines = lines  # First capture, send all

        # Update state
        state.last_hash = content_hash
        state.last_lines = lines
        state.sequence += 1

        if new_lines:
            return (new_lines, state.sequence)
        return None


class FeedPublisher:
    """Main feed publisher that coordinates watching and publishing"""

    def __init__(
        self,
        bind_address: str = "tcp://*:5555",
        project_root: Optional[Path] = None,
        session_name: Optional[str] = None,
        output_poll_interval: float = 0.5,
        stats_interval: float = 5.0,
    ):
        self.bind_address = bind_address
        self.project_root = project_root or Path.cwd()
        self.scud_dir = self.project_root / ".scud"
        self.session_name = session_name
        self.output_poll_interval = output_poll_interval
        self.stats_interval = stats_interval

        # ZMQ setup
        self.context = zmq.asyncio.Context()
        self.socket: Optional[zmq.asyncio.Socket] = None

        # Watchers
        self.session_watcher = SessionWatcher(self.scud_dir, self._on_session_change)
        self.output_captures: dict[str, TmuxOutputCapture] = {}

        # State tracking
        self._running = False
        self._sessions: dict[str, dict] = {}  # session_name -> session_data
        self._agent_statuses: dict[str, str] = {}  # task_id -> status

    async def start(self):
        """Start the feed publisher"""
        self.socket = self.context.socket(zmq.PUB)
        self.socket.setsockopt(zmq.SNDHWM, 1000)  # High water mark
        self.socket.bind(self.bind_address)

        # Give subscribers time to connect (ZMQ slow joiner issue)
        await asyncio.sleep(0.5)

        self._running = True
        print(f"Feed publisher started on {self.bind_address}")

        # Publish startup event
        await self._publish(FeedEvent(
            type="session",
            event="publisher_started",
            timestamp=self._now(),
            data={"bind_address": self.bind_address, "project_root": str(self.project_root)}
        ))

        # Start background tasks
        await asyncio.gather(
            self._session_watch_loop(),
            self._output_poll_loop(),
            self._stats_broadcast_loop(),
        )

    async def stop(self):
        """Stop the feed publisher"""
        self._running = False

        if self.socket:
            await self._publish(FeedEvent(
                type="session",
                event="publisher_stopped",
                timestamp=self._now(),
                data={}
            ))
            await asyncio.sleep(0.1)  # Flush
            self.socket.close()

        self.context.term()
        print("Feed publisher stopped")

    async def _publish(self, event: FeedEvent):
        """Publish an event to the feed"""
        if not self.socket:
            return

        topic = event.type.encode('utf-8')
        payload = event.to_json()

        try:
            await self.socket.send_multipart([topic, payload], flags=zmq.NOBLOCK)
        except zmq.Again:
            pass  # Socket buffer full, drop message

    def _now(self) -> str:
        """Get current UTC timestamp in ISO format"""
        return datetime.now(timezone.utc).isoformat()

    async def _session_watch_loop(self):
        """Watch for session file changes"""
        while self._running:
            try:
                changes = await self.session_watcher.check_sessions()

                for change_type, session_name, session_data in changes:
                    if self.session_name and session_name != self.session_name:
                        continue  # Filter to specific session

                    if change_type == "new":
                        self._sessions[session_name] = session_data
                        self.output_captures[session_name] = TmuxOutputCapture(session_name)
                        await self._publish(FeedEvent(
                            type="session",
                            event="started",
                            timestamp=self._now(),
                            data=session_data
                        ))
                        # Emit initial agent events
                        for agent in session_data.get("agents", []):
                            await self._publish(FeedEvent(
                                type="agent",
                                event="spawned",
                                timestamp=self._now(),
                                data=agent
                            ))
                            self._agent_statuses[agent["task_id"]] = agent.get("status", "starting")

                    elif change_type == "changed":
                        old_session = self._sessions.get(session_name, {})
                        self._sessions[session_name] = session_data

                        # Check for agent changes
                        old_agents = {a["task_id"]: a for a in old_session.get("agents", [])}
                        new_agents = {a["task_id"]: a for a in session_data.get("agents", [])}

                        # New agents
                        for task_id, agent in new_agents.items():
                            if task_id not in old_agents:
                                await self._publish(FeedEvent(
                                    type="agent",
                                    event="spawned",
                                    timestamp=self._now(),
                                    data=agent
                                ))
                                self._agent_statuses[task_id] = agent.get("status", "starting")
                            elif agent.get("status") != old_agents[task_id].get("status"):
                                old_status = self._agent_statuses.get(task_id)
                                new_status = agent.get("status")
                                self._agent_statuses[task_id] = new_status
                                await self._publish(FeedEvent(
                                    type="agent",
                                    event="status_changed",
                                    timestamp=self._now(),
                                    data={
                                        **agent,
                                        "previous_status": old_status
                                    }
                                ))

                    elif change_type == "removed":
                        if session_name in self._sessions:
                            del self._sessions[session_name]
                        if session_name in self.output_captures:
                            del self.output_captures[session_name]
                        await self._publish(FeedEvent(
                            type="session",
                            event="stopped",
                            timestamp=self._now(),
                            data={"session_name": session_name}
                        ))

            except Exception as e:
                print(f"Error in session watch loop: {e}")

            await asyncio.sleep(1.0)  # Check every second

    async def _output_poll_loop(self):
        """Poll tmux windows for output changes"""
        while self._running:
            try:
                for session_name, session_data in self._sessions.items():
                    capture = self.output_captures.get(session_name)
                    if not capture:
                        continue

                    for agent in session_data.get("agents", []):
                        task_id = agent["task_id"]
                        window_name = agent["window_name"]

                        result = capture.get_output_diff(task_id, window_name)
                        if result:
                            new_lines, sequence = result
                            # Batch lines to avoid flooding
                            if len(new_lines) > 50:
                                new_lines = new_lines[-50:]  # Last 50 lines

                            await self._publish(FeedEvent(
                                type="output",
                                event="lines",
                                timestamp=self._now(),
                                data={
                                    "task_id": task_id,
                                    "window_name": window_name,
                                    "lines": new_lines,
                                    "sequence": sequence
                                }
                            ))

            except Exception as e:
                print(f"Error in output poll loop: {e}")

            await asyncio.sleep(self.output_poll_interval)

    async def _stats_broadcast_loop(self):
        """Periodically broadcast session statistics"""
        while self._running:
            try:
                for session_name, session_data in self._sessions.items():
                    agents = session_data.get("agents", [])

                    # Count by status
                    status_counts = {
                        "starting": 0,
                        "running": 0,
                        "completed": 0,
                        "failed": 0
                    }
                    for agent in agents:
                        status = agent.get("status", "starting")
                        if status in status_counts:
                            status_counts[status] += 1

                    await self._publish(FeedEvent(
                        type="stats",
                        event="update",
                        timestamp=self._now(),
                        data={
                            "session_name": session_name,
                            "tag": session_data.get("tag", ""),
                            "total_agents": len(agents),
                            **status_counts
                        }
                    ))

            except Exception as e:
                print(f"Error in stats broadcast loop: {e}")

            await asyncio.sleep(self.stats_interval)

    async def _on_session_change(self, session_name: str, session_data: dict):
        """Callback for session file changes (used by watchdog if available)"""
        # This is called by watchdog for immediate notification
        # The polling loop handles the actual processing
        pass


async def main():
    parser = argparse.ArgumentParser(
        description="SCUD Monitor Feed Publisher - broadcast monitor events via ZMQ"
    )
    parser.add_argument(
        "--bind", "-b",
        default="tcp://*:5555",
        help="ZMQ bind address (default: tcp://*:5555)"
    )
    parser.add_argument(
        "--session", "-s",
        help="Monitor specific session only"
    )
    parser.add_argument(
        "--project", "-p",
        type=Path,
        help="Project root directory (default: current directory)"
    )
    parser.add_argument(
        "--output-interval",
        type=float,
        default=0.5,
        help="Output polling interval in seconds (default: 0.5)"
    )
    parser.add_argument(
        "--stats-interval",
        type=float,
        default=5.0,
        help="Stats broadcast interval in seconds (default: 5.0)"
    )

    args = parser.parse_args()

    publisher = FeedPublisher(
        bind_address=args.bind,
        project_root=args.project,
        session_name=args.session,
        output_poll_interval=args.output_interval,
        stats_interval=args.stats_interval,
    )

    try:
        await publisher.start()
    except KeyboardInterrupt:
        print("\nShutting down...")
    finally:
        await publisher.stop()


if __name__ == "__main__":
    asyncio.run(main())
