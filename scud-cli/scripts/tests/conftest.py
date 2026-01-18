"""
Pytest configuration and shared fixtures for socket feed tests.
"""

import asyncio
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import AsyncGenerator, Generator
from unittest.mock import MagicMock, patch

import pytest

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

import zmq
import zmq.asyncio

from feed_publisher import (
    AgentOutputState,
    FeedEvent,
    FeedPublisher,
    SessionWatcher,
    TmuxOutputCapture,
)
from feed_subscriber import Colors, EventAggregator, format_event_cli


# ============================================================================
# Event Loop Fixtures
# ============================================================================


@pytest.fixture(scope="session")
def event_loop_policy():
    """Use default event loop policy."""
    return asyncio.DefaultEventLoopPolicy()


@pytest.fixture
def event_loop():
    """Create an event loop for each test."""
    loop = asyncio.new_event_loop()
    yield loop
    loop.close()


# ============================================================================
# Temporary Directory Fixtures
# ============================================================================


@pytest.fixture
def temp_dir() -> Generator[Path, None, None]:
    """Create a temporary directory for test files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
def scud_dir(temp_dir: Path) -> Path:
    """Create a .scud directory structure."""
    scud = temp_dir / ".scud"
    scud.mkdir()
    (scud / "spawn").mkdir()
    return scud


@pytest.fixture
def project_root(scud_dir: Path) -> Path:
    """Return the project root (parent of .scud)."""
    return scud_dir.parent


# ============================================================================
# Sample Data Fixtures
# ============================================================================


@pytest.fixture
def sample_session_data() -> dict:
    """Sample session JSON data."""
    return {
        "session_name": "test-session",
        "tag": "test-tag",
        "working_dir": "/tmp/test-project",
        "terminal": "tmux",
        "agents": [
            {
                "task_id": "test:1",
                "task_title": "Test Task One",
                "window_name": "task-test:1",
                "status": "running",
                "started_at": "2024-01-15T10:00:00Z",
            },
            {
                "task_id": "test:2",
                "task_title": "Test Task Two",
                "window_name": "task-test:2",
                "status": "starting",
                "started_at": "2024-01-15T10:01:00Z",
            },
        ],
    }


@pytest.fixture
def sample_session_data_updated(sample_session_data: dict) -> dict:
    """Sample session data with status changes."""
    data = sample_session_data.copy()
    data["agents"] = [
        {
            "task_id": "test:1",
            "task_title": "Test Task One",
            "window_name": "task-test:1",
            "status": "completed",  # Changed from running
            "started_at": "2024-01-15T10:00:00Z",
            "completed_at": "2024-01-15T10:05:00Z",
        },
        {
            "task_id": "test:2",
            "task_title": "Test Task Two",
            "window_name": "task-test:2",
            "status": "running",  # Changed from starting
            "started_at": "2024-01-15T10:01:00Z",
        },
        {
            "task_id": "test:3",
            "task_title": "Test Task Three",  # New agent
            "window_name": "task-test:3",
            "status": "starting",
            "started_at": "2024-01-15T10:06:00Z",
        },
    ]
    return data


@pytest.fixture
def sample_feed_event() -> FeedEvent:
    """Sample FeedEvent for testing."""
    return FeedEvent(
        type="agent",
        event="status_changed",
        timestamp="2024-01-15T10:30:00Z",
        data={
            "task_id": "test:1",
            "status": "running",
            "previous_status": "starting",
        },
    )


@pytest.fixture
def sample_output_lines() -> list[str]:
    """Sample terminal output lines."""
    return [
        "$ scud next",
        "Finding next available task...",
        "Task: test:1 - Implement feature",
        "Dependencies met, ready to start.",
        "",
        "Starting task...",
        "Processing step 1/5...",
        "Processing step 2/5...",
        "Processing step 3/5...",
        "Processing step 4/5...",
        "Processing step 5/5...",
        "Task completed successfully!",
    ]


# ============================================================================
# Mock Fixtures
# ============================================================================


@pytest.fixture
def mock_subprocess():
    """Mock subprocess.run for tmux commands."""
    with patch("subprocess.run") as mock:
        yield mock


@pytest.fixture
def mock_tmux_list_windows(mock_subprocess):
    """Configure mock to return window list."""
    mock_subprocess.return_value = MagicMock(
        returncode=0,
        stdout="task-test:1\ntask-test:2\n",
    )
    return mock_subprocess


@pytest.fixture
def mock_tmux_capture_pane(mock_subprocess, sample_output_lines):
    """Configure mock to return captured pane output."""
    mock_subprocess.return_value = MagicMock(
        returncode=0,
        stdout="\n".join(sample_output_lines) + "\n",
    )
    return mock_subprocess


# ============================================================================
# ZMQ Fixtures
# ============================================================================


def get_free_port() -> int:
    """Get a free port for ZMQ binding."""
    import socket

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


@pytest.fixture
def zmq_port() -> int:
    """Get a free port for ZMQ tests."""
    return get_free_port()


@pytest.fixture
def zmq_address(zmq_port: int) -> str:
    """Get a ZMQ bind address."""
    return f"tcp://127.0.0.1:{zmq_port}"


@pytest.fixture
def zmq_context() -> Generator[zmq.asyncio.Context, None, None]:
    """Create a ZMQ context for tests."""
    ctx = zmq.asyncio.Context()
    yield ctx
    ctx.term()


@pytest.fixture
async def zmq_publisher(
    zmq_context: zmq.asyncio.Context, zmq_address: str
) -> AsyncGenerator[zmq.asyncio.Socket, None]:
    """Create a ZMQ PUB socket."""
    socket = zmq_context.socket(zmq.PUB)
    socket.bind(zmq_address)
    await asyncio.sleep(0.1)  # Allow socket to bind
    yield socket
    socket.close()


@pytest.fixture
async def zmq_subscriber(
    zmq_context: zmq.asyncio.Context, zmq_address: str
) -> AsyncGenerator[zmq.asyncio.Socket, None]:
    """Create a ZMQ SUB socket."""
    socket = zmq_context.socket(zmq.SUB)
    socket.connect(zmq_address)
    socket.subscribe(b"")  # Subscribe to all
    await asyncio.sleep(0.1)  # Allow socket to connect
    yield socket
    socket.close()


# ============================================================================
# Session File Helpers
# ============================================================================


@pytest.fixture
def write_session_file(scud_dir: Path):
    """Factory fixture to write session files."""

    def _write(session_name: str, data: dict) -> Path:
        path = scud_dir / "spawn" / f"{session_name}.json"
        path.write_text(json.dumps(data, indent=2))
        return path

    return _write


@pytest.fixture
def session_file(
    scud_dir: Path, sample_session_data: dict, write_session_file
) -> Path:
    """Create a session file with sample data."""
    return write_session_file("test-session", sample_session_data)


# ============================================================================
# Publisher/Subscriber Fixtures
# ============================================================================


@pytest.fixture
def session_watcher(scud_dir: Path) -> SessionWatcher:
    """Create a SessionWatcher instance."""
    return SessionWatcher(scud_dir, callback=None)


@pytest.fixture
def tmux_capture(sample_session_data: dict) -> TmuxOutputCapture:
    """Create a TmuxOutputCapture instance."""
    return TmuxOutputCapture(
        session_name=sample_session_data["session_name"],
        max_lines=100,
    )


@pytest.fixture
def event_aggregator() -> EventAggregator:
    """Create an EventAggregator instance."""
    return EventAggregator()


# ============================================================================
# Async Test Helpers
# ============================================================================


async def wait_for_condition(
    condition_fn,
    timeout: float = 2.0,
    interval: float = 0.05,
) -> bool:
    """Wait for a condition to become true."""
    import time

    start = time.time()
    while time.time() - start < timeout:
        if condition_fn():
            return True
        await asyncio.sleep(interval)
    return False


async def collect_events(
    socket: zmq.asyncio.Socket,
    count: int = 1,
    timeout: float = 2.0,
) -> list[dict]:
    """Collect events from a ZMQ socket."""
    events = []
    try:
        async with asyncio.timeout(timeout):
            while len(events) < count:
                topic, payload = await socket.recv_multipart()
                events.append(json.loads(payload.decode()))
    except asyncio.TimeoutError:
        pass
    return events
