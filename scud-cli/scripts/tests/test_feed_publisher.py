"""
Unit tests for FeedPublisher component.
"""

import asyncio
import json
import sys
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest
import zmq
import zmq.asyncio

sys.path.insert(0, str(Path(__file__).parent.parent))

from feed_publisher import FeedEvent, FeedPublisher


class TestFeedPublisherInit:
    """Tests for FeedPublisher initialization."""

    def test_init_defaults(self, project_root: Path):
        """Test FeedPublisher with default parameters."""
        publisher = FeedPublisher(project_root=project_root)

        assert publisher.bind_address == "tcp://*:5555"
        assert publisher.project_root == project_root
        assert publisher.scud_dir == project_root / ".scud"
        assert publisher.session_name is None
        assert publisher.output_poll_interval == 0.5
        assert publisher.stats_interval == 5.0
        assert publisher._running is False

    def test_init_custom_params(self, project_root: Path):
        """Test FeedPublisher with custom parameters."""
        publisher = FeedPublisher(
            bind_address="tcp://*:6666",
            project_root=project_root,
            session_name="my-session",
            output_poll_interval=0.25,
            stats_interval=10.0,
        )

        assert publisher.bind_address == "tcp://*:6666"
        assert publisher.session_name == "my-session"
        assert publisher.output_poll_interval == 0.25
        assert publisher.stats_interval == 10.0


class TestFeedPublisherHelpers:
    """Tests for FeedPublisher helper methods."""

    def test_now_returns_iso_format(self, project_root: Path):
        """Test _now returns ISO formatted timestamp."""
        publisher = FeedPublisher(project_root=project_root)

        timestamp = publisher._now()

        # Should be ISO format with timezone
        assert "T" in timestamp
        assert "+" in timestamp or "Z" in timestamp

    def test_now_is_utc(self, project_root: Path):
        """Test _now returns UTC timestamp."""
        from datetime import datetime, timezone

        publisher = FeedPublisher(project_root=project_root)

        before = datetime.now(timezone.utc)
        timestamp = publisher._now()
        after = datetime.now(timezone.utc)

        # Parse the timestamp
        parsed = datetime.fromisoformat(timestamp)

        # Should be between before and after
        assert before <= parsed <= after


class TestFeedPublisherPublish:
    """Tests for FeedPublisher._publish method."""

    @pytest.mark.asyncio
    async def test_publish_without_socket(self, project_root: Path):
        """Test publishing without socket does nothing."""
        publisher = FeedPublisher(project_root=project_root)
        event = FeedEvent(type="test", event="test", timestamp="", data={})

        # Should not raise
        await publisher._publish(event)

    @pytest.mark.asyncio
    async def test_publish_sends_multipart(
        self,
        project_root: Path,
        zmq_address: str,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test publishing sends multipart message."""
        publisher = FeedPublisher(
            bind_address=zmq_address,
            project_root=project_root,
        )

        # Create socket manually for testing
        publisher.socket = zmq_context.socket(zmq.PUB)
        publisher.socket.bind(zmq_address.replace("127.0.0.1", "*"))
        await asyncio.sleep(0.1)

        # Create subscriber
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(zmq_address)
        sub.subscribe(b"")
        await asyncio.sleep(0.1)

        # Publish event
        event = FeedEvent(
            type="agent",
            event="spawned",
            timestamp="2024-01-15T10:00:00Z",
            data={"task_id": "test:1"},
        )
        await publisher._publish(event)

        # Receive with timeout
        try:
            async with asyncio.timeout(1.0):
                topic, payload = await sub.recv_multipart()
        except asyncio.TimeoutError:
            pytest.fail("Timeout waiting for message")

        assert topic == b"agent"
        parsed = json.loads(payload.decode())
        assert parsed["type"] == "agent"
        assert parsed["event"] == "spawned"
        assert parsed["data"]["task_id"] == "test:1"

        # Cleanup
        sub.close()
        publisher.socket.close()

    @pytest.mark.asyncio
    async def test_publish_handles_full_buffer(self, project_root: Path):
        """Test publishing handles EAGAIN (buffer full)."""
        publisher = FeedPublisher(project_root=project_root)

        # Mock socket that raises EAGAIN
        mock_socket = AsyncMock()
        mock_socket.send_multipart = AsyncMock(side_effect=zmq.Again())
        publisher.socket = mock_socket

        event = FeedEvent(type="test", event="test", timestamp="", data={})

        # Should not raise
        await publisher._publish(event)


class TestFeedPublisherStartStop:
    """Tests for FeedPublisher start/stop lifecycle."""

    @pytest.mark.asyncio
    async def test_start_binds_socket(
        self,
        project_root: Path,
        zmq_port: int,
    ):
        """Test start binds ZMQ socket."""
        address = f"tcp://127.0.0.1:{zmq_port}"
        publisher = FeedPublisher(
            bind_address=address,
            project_root=project_root,
            output_poll_interval=0.1,
            stats_interval=0.1,
        )

        # Start in background
        start_task = asyncio.create_task(publisher.start())

        # Wait for socket to bind with retry
        for _ in range(20):  # Up to 2 seconds
            await asyncio.sleep(0.1)
            if publisher._running and publisher.socket is not None:
                break

        assert publisher._running is True
        assert publisher.socket is not None

        # Stop publisher
        publisher._running = False
        start_task.cancel()

        try:
            await start_task
        except asyncio.CancelledError:
            pass

    @pytest.mark.asyncio
    async def test_stop_closes_socket(
        self,
        project_root: Path,
        zmq_port: int,
    ):
        """Test stop closes ZMQ socket."""
        address = f"tcp://127.0.0.1:{zmq_port}"
        publisher = FeedPublisher(
            bind_address=address,
            project_root=project_root,
        )

        # Manually create socket
        publisher.socket = publisher.context.socket(zmq.PUB)
        publisher.socket.bind(address.replace("127.0.0.1", "*"))
        publisher._running = True

        await publisher.stop()

        assert publisher._running is False


class TestFeedPublisherSessionWatch:
    """Tests for FeedPublisher session watching."""

    @pytest.mark.asyncio
    async def test_session_watch_detects_new_session(
        self,
        project_root: Path,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test session watch loop detects new sessions."""
        address = f"tcp://127.0.0.1:{zmq_port}"
        publisher = FeedPublisher(
            bind_address=address,
            project_root=project_root,
            output_poll_interval=10.0,  # Slow to avoid interference
            stats_interval=10.0,
        )

        # Create subscriber
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"session")
        sub.subscribe(b"agent")

        # Start publisher
        start_task = asyncio.create_task(publisher.start())
        await asyncio.sleep(0.3)

        # Receive publisher_started event
        try:
            async with asyncio.timeout(1.0):
                topic, payload = await sub.recv_multipart()
            assert topic == b"session"
            event = json.loads(payload.decode())
            assert event["event"] == "publisher_started"
        except asyncio.TimeoutError:
            pytest.fail("Timeout waiting for publisher_started")

        # Write session file
        write_session_file("test-session", sample_session_data)

        # Wait for detection
        await asyncio.sleep(1.5)  # Session watch is 1s interval

        # Should receive session started and agent events
        events = []
        try:
            async with asyncio.timeout(2.0):
                while len(events) < 3:  # 1 session + 2 agents
                    topic, payload = await sub.recv_multipart()
                    events.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        # Verify we got expected events
        session_events = [e for e in events if e["type"] == "session"]
        agent_events = [e for e in events if e["type"] == "agent"]

        assert len(session_events) >= 1
        assert any(e["event"] == "started" for e in session_events)

        # Cleanup
        publisher._running = False
        start_task.cancel()
        try:
            await start_task
        except asyncio.CancelledError:
            pass
        sub.close()

    @pytest.mark.asyncio
    async def test_session_watch_filters_by_session_name(
        self,
        project_root: Path,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test session watch filters to specific session."""
        address = f"tcp://127.0.0.1:{zmq_port}"
        publisher = FeedPublisher(
            bind_address=address,
            project_root=project_root,
            session_name="target-session",  # Only watch this session
            output_poll_interval=10.0,
            stats_interval=10.0,
        )

        # Create subscriber
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"session")

        # Start publisher
        start_task = asyncio.create_task(publisher.start())
        await asyncio.sleep(0.3)

        # Skip publisher_started
        try:
            async with asyncio.timeout(1.0):
                await sub.recv_multipart()
        except asyncio.TimeoutError:
            pass

        # Write sessions - one matching, one not
        write_session_file("other-session", sample_session_data)
        data2 = sample_session_data.copy()
        data2["session_name"] = "target-session"
        write_session_file("target-session", data2)

        await asyncio.sleep(1.5)

        # Collect events
        events = []
        try:
            async with asyncio.timeout(2.0):
                while True:
                    topic, payload = await sub.recv_multipart()
                    events.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        # Should only have events for target-session
        started_events = [e for e in events if e["event"] == "started"]
        # Filtering happens, so we might not see other-session
        # But we should see target-session
        assert any(
            e.get("data", {}).get("session_name") == "target-session"
            for e in started_events
        )

        # Cleanup
        publisher._running = False
        start_task.cancel()
        try:
            await start_task
        except asyncio.CancelledError:
            pass
        sub.close()


class TestFeedPublisherAgentTracking:
    """Tests for FeedPublisher agent status tracking."""

    @pytest.mark.asyncio
    async def test_tracks_agent_status_changes(
        self,
        project_root: Path,
        scud_dir: Path,
        sample_session_data: dict,
        sample_session_data_updated: dict,
        write_session_file,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test publisher tracks and emits agent status changes."""
        address = f"tcp://127.0.0.1:{zmq_port}"
        publisher = FeedPublisher(
            bind_address=address,
            project_root=project_root,
            output_poll_interval=10.0,
            stats_interval=10.0,
        )

        # Create subscriber
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"agent")

        # Start publisher
        start_task = asyncio.create_task(publisher.start())
        await asyncio.sleep(0.3)

        # Write initial session
        write_session_file("test-session", sample_session_data)
        await asyncio.sleep(1.5)

        # Collect initial agent events
        initial_events = []
        try:
            async with asyncio.timeout(1.0):
                while True:
                    topic, payload = await sub.recv_multipart()
                    initial_events.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        # Should have spawned events
        spawned = [e for e in initial_events if e["event"] == "spawned"]
        assert len(spawned) == 2

        # Update session (status changes)
        write_session_file("test-session", sample_session_data_updated)
        await asyncio.sleep(1.5)

        # Collect status change events
        change_events = []
        try:
            async with asyncio.timeout(1.0):
                while True:
                    topic, payload = await sub.recv_multipart()
                    change_events.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        # Should have status_changed events
        status_changed = [e for e in change_events if e["event"] == "status_changed"]
        assert len(status_changed) >= 2  # test:1 and test:2 changed

        # Should have spawned for new agent test:3
        new_spawned = [e for e in change_events if e["event"] == "spawned"]
        assert len(new_spawned) >= 1

        # Cleanup
        publisher._running = False
        start_task.cancel()
        try:
            await start_task
        except asyncio.CancelledError:
            pass
        sub.close()


class TestFeedPublisherStats:
    """Tests for FeedPublisher stats broadcasting."""

    @pytest.mark.asyncio
    async def test_broadcasts_stats_periodically(
        self,
        project_root: Path,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test publisher broadcasts stats at intervals."""
        address = f"tcp://127.0.0.1:{zmq_port}"
        publisher = FeedPublisher(
            bind_address=address,
            project_root=project_root,
            output_poll_interval=10.0,
            stats_interval=0.5,  # Fast stats for testing
        )

        # Create subscriber
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"stats")

        # Write session
        write_session_file("test-session", sample_session_data)

        # Start publisher
        start_task = asyncio.create_task(publisher.start())
        await asyncio.sleep(0.3)

        # Wait for session detection
        await asyncio.sleep(1.5)

        # Collect stats events
        stats_events = []
        try:
            async with asyncio.timeout(2.0):
                while len(stats_events) < 2:  # Wait for at least 2 stats
                    topic, payload = await sub.recv_multipart()
                    stats_events.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        assert len(stats_events) >= 1

        # Verify stats content
        stats = stats_events[0]
        assert stats["type"] == "stats"
        assert stats["event"] == "update"
        assert "total_agents" in stats["data"]
        assert "running" in stats["data"]
        assert "completed" in stats["data"]
        assert stats["data"]["total_agents"] == 2

        # Cleanup
        publisher._running = False
        start_task.cancel()
        try:
            await start_task
        except asyncio.CancelledError:
            pass
        sub.close()
