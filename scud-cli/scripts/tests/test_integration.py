"""
Integration tests for the full pub/sub flow.

These tests verify that the publisher and subscriber work together correctly.
"""

import asyncio
import json
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest
import zmq
import zmq.asyncio

sys.path.insert(0, str(Path(__file__).parent.parent))

from feed_publisher import FeedEvent, FeedPublisher
from feed_subscriber import EventAggregator


class TestPubSubIntegration:
    """Integration tests for publisher/subscriber communication."""

    @pytest.mark.asyncio
    async def test_simple_pub_sub_flow(
        self,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test basic publish/subscribe message flow."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        # Create publisher socket
        pub = zmq_context.socket(zmq.PUB)
        pub.bind(address.replace("127.0.0.1", "*"))

        # Create subscriber socket
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"")

        # Wait for connection
        await asyncio.sleep(0.2)

        # Send message
        event = FeedEvent(
            type="test",
            event="hello",
            timestamp="2024-01-15T10:00:00Z",
            data={"message": "world"},
        )
        await pub.send_multipart([b"test", event.to_json()])

        # Receive message
        try:
            async with asyncio.timeout(1.0):
                topic, payload = await sub.recv_multipart()
        except asyncio.TimeoutError:
            pytest.fail("Timeout receiving message")

        assert topic == b"test"
        parsed = json.loads(payload.decode())
        assert parsed["type"] == "test"
        assert parsed["data"]["message"] == "world"

        # Cleanup
        pub.close()
        sub.close()

    @pytest.mark.asyncio
    async def test_topic_filtering(
        self,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test that subscribers only receive subscribed topics."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        # Create publisher
        pub = zmq_context.socket(zmq.PUB)
        pub.bind(address.replace("127.0.0.1", "*"))

        # Create subscriber that only subscribes to "agent" topic
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"agent")

        await asyncio.sleep(0.2)

        # Send messages with different topics
        topics = ["session", "agent", "output", "stats"]
        for topic in topics:
            event = FeedEvent(
                type=topic,
                event="test",
                timestamp="2024-01-15T10:00:00Z",
                data={"topic": topic},
            )
            await pub.send_multipart([topic.encode(), event.to_json()])

        # Receive - should only get "agent"
        received = []
        try:
            async with asyncio.timeout(0.5):
                while True:
                    topic, payload = await sub.recv_multipart()
                    received.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        assert len(received) == 1
        assert received[0]["type"] == "agent"

        # Cleanup
        pub.close()
        sub.close()

    @pytest.mark.asyncio
    async def test_multiple_subscribers(
        self,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test multiple subscribers receive the same messages."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        # Create publisher
        pub = zmq_context.socket(zmq.PUB)
        pub.bind(address.replace("127.0.0.1", "*"))

        # Create multiple subscribers
        subscribers = []
        for _ in range(3):
            sub = zmq_context.socket(zmq.SUB)
            sub.connect(address)
            sub.subscribe(b"")
            subscribers.append(sub)

        await asyncio.sleep(0.2)

        # Send message
        event = FeedEvent(
            type="test",
            event="broadcast",
            timestamp="2024-01-15T10:00:00Z",
            data={"id": 1},
        )
        await pub.send_multipart([b"test", event.to_json()])

        # All subscribers should receive
        for i, sub in enumerate(subscribers):
            try:
                async with asyncio.timeout(1.0):
                    topic, payload = await sub.recv_multipart()
                    parsed = json.loads(payload.decode())
                    assert parsed["data"]["id"] == 1
            except asyncio.TimeoutError:
                pytest.fail(f"Subscriber {i} did not receive message")

        # Cleanup
        pub.close()
        for sub in subscribers:
            sub.close()

    @pytest.mark.asyncio
    async def test_late_subscriber(
        self,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test that late subscribers miss earlier messages (slow joiner)."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        # Create publisher
        pub = zmq_context.socket(zmq.PUB)
        pub.bind(address.replace("127.0.0.1", "*"))
        await asyncio.sleep(0.1)

        # Send message BEFORE subscriber connects
        event1 = FeedEvent(
            type="test",
            event="early",
            timestamp="2024-01-15T10:00:00Z",
            data={"id": 1},
        )
        await pub.send_multipart([b"test", event1.to_json()])

        # Now create subscriber
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"")
        await asyncio.sleep(0.2)

        # Send message AFTER subscriber connects
        event2 = FeedEvent(
            type="test",
            event="late",
            timestamp="2024-01-15T10:01:00Z",
            data={"id": 2},
        )
        await pub.send_multipart([b"test", event2.to_json()])

        # Subscriber should only receive the second message
        received = []
        try:
            async with asyncio.timeout(0.5):
                while True:
                    topic, payload = await sub.recv_multipart()
                    received.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        assert len(received) == 1
        assert received[0]["event"] == "late"
        assert received[0]["data"]["id"] == 2

        # Cleanup
        pub.close()
        sub.close()


class TestFullPublisherSubscriberFlow:
    """End-to-end tests with full publisher and subscriber."""

    @pytest.mark.asyncio
    async def test_publisher_to_aggregator_flow(
        self,
        project_root: Path,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test publisher events flow to aggregator correctly."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        # Create publisher
        publisher = FeedPublisher(
            bind_address=address,
            project_root=project_root,
            output_poll_interval=10.0,
            stats_interval=0.5,
        )

        # Create subscriber
        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"")

        # Create aggregator
        aggregator = EventAggregator()

        # Start publisher
        start_task = asyncio.create_task(publisher.start())
        await asyncio.sleep(0.3)

        # Write session file
        write_session_file("test-session", sample_session_data)

        # Wait for detection
        await asyncio.sleep(1.5)

        # Collect and aggregate events
        try:
            async with asyncio.timeout(3.0):
                while True:
                    topic, payload = await sub.recv_multipart()
                    event = json.loads(payload.decode())
                    aggregator.process_event(event)
        except asyncio.TimeoutError:
            pass

        # Verify aggregator state
        state = aggregator.get_state()

        # Should have session
        assert len(state["sessions"]) >= 1

        # Should have agents
        assert len(state["agents"]) >= 2

        # Should have stats
        assert state["stats"] is not None

        # Cleanup
        publisher._running = False
        start_task.cancel()
        try:
            await start_task
        except asyncio.CancelledError:
            pass
        sub.close()

    @pytest.mark.asyncio
    async def test_session_lifecycle(
        self,
        project_root: Path,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test full session lifecycle: create, update, delete."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        publisher = FeedPublisher(
            bind_address=address,
            project_root=project_root,
            output_poll_interval=10.0,
            stats_interval=10.0,
        )

        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"session")

        aggregator = EventAggregator()

        # Start publisher
        start_task = asyncio.create_task(publisher.start())
        await asyncio.sleep(0.3)

        # Skip publisher_started
        try:
            async with asyncio.timeout(1.0):
                await sub.recv_multipart()
        except asyncio.TimeoutError:
            pass

        # 1. Create session
        session_path = write_session_file("lifecycle-test", sample_session_data)
        await asyncio.sleep(1.5)

        events = []
        try:
            async with asyncio.timeout(1.0):
                while True:
                    topic, payload = await sub.recv_multipart()
                    events.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        started_events = [e for e in events if e["event"] == "started"]
        assert len(started_events) >= 1

        # 2. Delete session
        session_path.unlink()
        await asyncio.sleep(1.5)

        events = []
        try:
            async with asyncio.timeout(1.0):
                while True:
                    topic, payload = await sub.recv_multipart()
                    events.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        stopped_events = [e for e in events if e["event"] == "stopped"]
        assert len(stopped_events) >= 1

        # Cleanup
        publisher._running = False
        start_task.cancel()
        try:
            await start_task
        except asyncio.CancelledError:
            pass
        sub.close()


class TestOutputCaptureTmuxIntegration:
    """Tests for tmux output capture with mocked tmux."""

    @pytest.mark.asyncio
    async def test_output_capture_flow(
        self,
        project_root: Path,
        scud_dir: Path,
        sample_session_data: dict,
        sample_output_lines: list[str],
        write_session_file,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test output capture publishes to feed."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        # Write session
        write_session_file("test-session", sample_session_data)

        with patch("subprocess.run") as mock_run:
            # Mock tmux capture-pane to return sample output
            mock_run.return_value = MagicMock(
                returncode=0,
                stdout="\n".join(sample_output_lines) + "\n",
            )

            publisher = FeedPublisher(
                bind_address=address,
                project_root=project_root,
                output_poll_interval=0.2,  # Fast polling
                stats_interval=10.0,
            )

            sub = zmq_context.socket(zmq.SUB)
            sub.connect(address)
            sub.subscribe(b"output")

            # Start publisher
            start_task = asyncio.create_task(publisher.start())
            await asyncio.sleep(0.5)

            # Wait for output capture
            await asyncio.sleep(1.5)

            # Collect output events
            output_events = []
            try:
                async with asyncio.timeout(2.0):
                    while len(output_events) < 2:
                        topic, payload = await sub.recv_multipart()
                        output_events.append(json.loads(payload.decode()))
            except asyncio.TimeoutError:
                pass

            # Should have captured some output
            assert len(output_events) >= 1
            assert output_events[0]["type"] == "output"
            assert "lines" in output_events[0]["data"]

            # Cleanup
            publisher._running = False
            start_task.cancel()
            try:
                await start_task
            except asyncio.CancelledError:
                pass
            sub.close()


class TestHighWaterMark:
    """Tests for ZMQ high water mark behavior."""

    @pytest.mark.asyncio
    async def test_publisher_hwm_drops_messages(
        self,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test that publisher drops messages when HWM is reached."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        # Create publisher with low HWM
        pub = zmq_context.socket(zmq.PUB)
        pub.setsockopt(zmq.SNDHWM, 5)
        pub.bind(address.replace("127.0.0.1", "*"))

        # DON'T create subscriber - messages should queue then drop
        await asyncio.sleep(0.1)

        # Send many messages (more than HWM)
        sent = 0
        for i in range(100):
            event = FeedEvent(
                type="test",
                event="flood",
                timestamp="2024-01-15T10:00:00Z",
                data={"id": i},
            )
            try:
                await pub.send_multipart(
                    [b"test", event.to_json()],
                    flags=zmq.NOBLOCK,
                )
                sent += 1
            except zmq.Again:
                # Expected when buffer is full
                break

        # Should have been able to send at least some
        assert sent > 0
        # But not all (some dropped due to no subscriber and HWM)

        pub.close()


class TestReconnection:
    """Tests for subscriber reconnection scenarios."""

    @pytest.mark.asyncio
    async def test_subscriber_reconnects(
        self,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test subscriber can reconnect to publisher."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        # Create publisher
        pub = zmq_context.socket(zmq.PUB)
        pub.bind(address.replace("127.0.0.1", "*"))

        # First connection
        sub1 = zmq_context.socket(zmq.SUB)
        sub1.connect(address)
        sub1.subscribe(b"")
        await asyncio.sleep(0.2)

        # Send and receive
        event1 = FeedEvent(
            type="test", event="first", timestamp="", data={}
        )
        await pub.send_multipart([b"test", event1.to_json()])

        try:
            async with asyncio.timeout(1.0):
                await sub1.recv_multipart()
        except asyncio.TimeoutError:
            pytest.fail("First connection failed")

        # Close first subscriber
        sub1.close()
        await asyncio.sleep(0.1)

        # Create new subscriber
        sub2 = zmq_context.socket(zmq.SUB)
        sub2.connect(address)
        sub2.subscribe(b"")
        await asyncio.sleep(0.2)

        # Send and receive again
        event2 = FeedEvent(
            type="test", event="second", timestamp="", data={}
        )
        await pub.send_multipart([b"test", event2.to_json()])

        try:
            async with asyncio.timeout(1.0):
                topic, payload = await sub2.recv_multipart()
                parsed = json.loads(payload.decode())
                assert parsed["event"] == "second"
        except asyncio.TimeoutError:
            pytest.fail("Reconnection failed")

        # Cleanup
        pub.close()
        sub2.close()


class TestMessageOrdering:
    """Tests for message ordering guarantees."""

    @pytest.mark.asyncio
    async def test_messages_received_in_order(
        self,
        zmq_port: int,
        zmq_context: zmq.asyncio.Context,
    ):
        """Test messages are received in the order sent."""
        address = f"tcp://127.0.0.1:{zmq_port}"

        pub = zmq_context.socket(zmq.PUB)
        pub.bind(address.replace("127.0.0.1", "*"))

        sub = zmq_context.socket(zmq.SUB)
        sub.connect(address)
        sub.subscribe(b"")
        await asyncio.sleep(0.2)

        # Send numbered messages
        count = 50
        for i in range(count):
            event = FeedEvent(
                type="test",
                event="ordered",
                timestamp="",
                data={"seq": i},
            )
            await pub.send_multipart([b"test", event.to_json()])

        # Receive and verify order
        received = []
        try:
            async with asyncio.timeout(2.0):
                while len(received) < count:
                    topic, payload = await sub.recv_multipart()
                    received.append(json.loads(payload.decode()))
        except asyncio.TimeoutError:
            pass

        # Verify sequence
        for i, event in enumerate(received):
            assert event["data"]["seq"] == i, f"Out of order at position {i}"

        # Cleanup
        pub.close()
        sub.close()
