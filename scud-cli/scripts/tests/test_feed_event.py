"""
Unit tests for FeedEvent and related data classes.
"""

import json
import sys
from dataclasses import asdict
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent.parent))

from feed_publisher import AgentOutputState, FeedEvent


class TestFeedEvent:
    """Tests for the FeedEvent dataclass."""

    def test_create_feed_event(self):
        """Test creating a FeedEvent with all fields."""
        event = FeedEvent(
            type="agent",
            event="spawned",
            timestamp="2024-01-15T10:00:00Z",
            data={"task_id": "test:1", "status": "starting"},
        )

        assert event.type == "agent"
        assert event.event == "spawned"
        assert event.timestamp == "2024-01-15T10:00:00Z"
        assert event.data == {"task_id": "test:1", "status": "starting"}

    def test_feed_event_to_json(self):
        """Test serializing FeedEvent to JSON bytes."""
        event = FeedEvent(
            type="output",
            event="lines",
            timestamp="2024-01-15T10:00:00Z",
            data={"lines": ["line1", "line2"], "sequence": 1},
        )

        json_bytes = event.to_json()

        assert isinstance(json_bytes, bytes)
        parsed = json.loads(json_bytes.decode("utf-8"))
        assert parsed["type"] == "output"
        assert parsed["event"] == "lines"
        assert parsed["data"]["lines"] == ["line1", "line2"]
        assert parsed["data"]["sequence"] == 1

    def test_feed_event_to_json_unicode(self):
        """Test FeedEvent handles unicode characters correctly."""
        event = FeedEvent(
            type="output",
            event="lines",
            timestamp="2024-01-15T10:00:00Z",
            data={"lines": ["Hello 世界", "Émoji: 🚀"]},
        )

        json_bytes = event.to_json()
        parsed = json.loads(json_bytes.decode("utf-8"))

        assert parsed["data"]["lines"][0] == "Hello 世界"
        assert parsed["data"]["lines"][1] == "Émoji: 🚀"

    def test_feed_event_to_json_empty_data(self):
        """Test FeedEvent with empty data dictionary."""
        event = FeedEvent(
            type="session",
            event="stopped",
            timestamp="2024-01-15T10:00:00Z",
            data={},
        )

        json_bytes = event.to_json()
        parsed = json.loads(json_bytes.decode("utf-8"))

        assert parsed["data"] == {}

    def test_feed_event_to_json_nested_data(self):
        """Test FeedEvent with nested data structures."""
        event = FeedEvent(
            type="stats",
            event="update",
            timestamp="2024-01-15T10:00:00Z",
            data={
                "session": {"name": "test", "tag": "feature"},
                "counts": {"running": 3, "completed": 5},
                "agents": [
                    {"id": "1", "status": "running"},
                    {"id": "2", "status": "completed"},
                ],
            },
        )

        json_bytes = event.to_json()
        parsed = json.loads(json_bytes.decode("utf-8"))

        assert parsed["data"]["session"]["name"] == "test"
        assert parsed["data"]["counts"]["running"] == 3
        assert len(parsed["data"]["agents"]) == 2

    def test_feed_event_asdict(self):
        """Test converting FeedEvent to dictionary."""
        event = FeedEvent(
            type="agent",
            event="status_changed",
            timestamp="2024-01-15T10:00:00Z",
            data={"task_id": "test:1"},
        )

        d = asdict(event)

        assert d == {
            "type": "agent",
            "event": "status_changed",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"task_id": "test:1"},
        }

    @pytest.mark.parametrize(
        "event_type,event_name",
        [
            ("session", "started"),
            ("session", "stopped"),
            ("session", "publisher_started"),
            ("session", "publisher_stopped"),
            ("agent", "spawned"),
            ("agent", "status_changed"),
            ("output", "lines"),
            ("stats", "update"),
            ("task", "status_changed"),
        ],
    )
    def test_feed_event_valid_types(self, event_type: str, event_name: str):
        """Test all valid event type/name combinations."""
        event = FeedEvent(
            type=event_type,
            event=event_name,
            timestamp="2024-01-15T10:00:00Z",
            data={},
        )

        assert event.type == event_type
        assert event.event == event_name


class TestAgentOutputState:
    """Tests for the AgentOutputState dataclass."""

    def test_create_agent_output_state(self):
        """Test creating AgentOutputState with required fields."""
        state = AgentOutputState(
            task_id="test:1",
            window_name="task-test:1",
        )

        assert state.task_id == "test:1"
        assert state.window_name == "task-test:1"
        assert state.last_hash == ""
        assert state.sequence == 0
        assert state.last_lines == []

    def test_agent_output_state_with_all_fields(self):
        """Test creating AgentOutputState with all fields."""
        state = AgentOutputState(
            task_id="test:1",
            window_name="task-test:1",
            last_hash="abc123",
            sequence=5,
            last_lines=["line1", "line2"],
        )

        assert state.last_hash == "abc123"
        assert state.sequence == 5
        assert state.last_lines == ["line1", "line2"]

    def test_agent_output_state_mutable_fields(self):
        """Test that mutable fields are properly initialized."""
        state1 = AgentOutputState(task_id="test:1", window_name="w1")
        state2 = AgentOutputState(task_id="test:2", window_name="w2")

        # Modify state1's list
        state1.last_lines.append("line")

        # Ensure state2's list is not affected (no shared mutable default)
        assert state1.last_lines == ["line"]
        assert state2.last_lines == []

    def test_agent_output_state_update_sequence(self):
        """Test updating sequence number."""
        state = AgentOutputState(task_id="test:1", window_name="w1")

        state.sequence += 1
        assert state.sequence == 1

        state.sequence += 1
        assert state.sequence == 2
