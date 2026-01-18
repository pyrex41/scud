"""
Unit tests for feed subscriber components.
"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent.parent))

from feed_subscriber import Colors, EventAggregator, format_event_cli


class TestColors:
    """Tests for Colors utility class."""

    def test_reset_code(self):
        """Test RESET color code."""
        assert Colors.RESET == "\033[0m"

    def test_color_codes_exist(self):
        """Test all color codes are defined."""
        assert hasattr(Colors, "RED")
        assert hasattr(Colors, "GREEN")
        assert hasattr(Colors, "YELLOW")
        assert hasattr(Colors, "BLUE")
        assert hasattr(Colors, "CYAN")
        assert hasattr(Colors, "MAGENTA")
        assert hasattr(Colors, "WHITE")
        assert hasattr(Colors, "BOLD")
        assert hasattr(Colors, "DIM")

    def test_colored_wraps_text(self):
        """Test colored method wraps text with color codes."""
        result = Colors.colored("test", Colors.RED)

        assert result.startswith(Colors.RED)
        assert result.endswith(Colors.RESET)
        assert "test" in result

    def test_colored_with_bold(self):
        """Test colored with bold style."""
        result = Colors.colored("text", Colors.BOLD)

        assert Colors.BOLD in result
        assert Colors.RESET in result


class TestFormatEventCli:
    """Tests for format_event_cli function."""

    def test_format_output_event(self):
        """Test formatting output event."""
        event = {
            "type": "output",
            "event": "lines",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {
                "task_id": "test:1",
                "lines": ["Processing...", "Done!"],
                "sequence": 5,
            },
        }

        result = format_event_cli(event)

        assert "test:1" in result
        assert "seq=5" in result
        assert "Processing..." in result
        assert "Done!" in result

    def test_format_output_event_truncates_lines(self):
        """Test output event shows only last 10 lines."""
        event = {
            "type": "output",
            "event": "lines",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {
                "task_id": "test:1",
                "lines": [f"line {i}" for i in range(20)],
                "sequence": 1,
            },
        }

        result = format_event_cli(event)

        # Should only have last 10 lines
        assert "line 10" in result
        assert "line 19" in result
        # First lines should not be present
        assert "line 0" not in result

    def test_format_agent_event_spawned(self):
        """Test formatting agent spawned event."""
        event = {
            "type": "agent",
            "event": "spawned",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {
                "task_id": "test:1",
                "status": "starting",
            },
        }

        result = format_event_cli(event)

        assert "AGENT" in result
        assert "spawned" in result
        assert "test:1" in result
        assert "starting" in result

    def test_format_agent_event_status_changed(self):
        """Test formatting agent status changed event."""
        event = {
            "type": "agent",
            "event": "status_changed",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {
                "task_id": "test:1",
                "status": "running",
                "previous_status": "starting",
            },
        }

        result = format_event_cli(event)

        assert "AGENT" in result
        assert "status_changed" in result
        assert "test:1" in result
        assert "starting" in result
        assert "->" in result
        assert "running" in result

    def test_format_stats_event(self):
        """Test formatting stats event."""
        event = {
            "type": "stats",
            "event": "update",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {
                "session_name": "my-session",
                "total_agents": 5,
                "running": 2,
                "completed": 2,
                "failed": 1,
            },
        }

        result = format_event_cli(event)

        assert "STATS" in result
        assert "my-session" in result
        assert "2" in result  # running/completed
        assert "5 total" in result

    def test_format_session_event(self):
        """Test formatting session event."""
        event = {
            "type": "session",
            "event": "started",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {
                "session_name": "test-session",
            },
        }

        result = format_event_cli(event)

        assert "SESSION" in result
        assert "started" in result
        assert "test-session" in result

    def test_format_session_publisher_started(self):
        """Test formatting publisher started event."""
        event = {
            "type": "session",
            "event": "publisher_started",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {
                "bind_address": "tcp://*:5555",
            },
        }

        result = format_event_cli(event)

        assert "SESSION" in result
        assert "publisher_started" in result
        assert "tcp://*:5555" in result

    def test_format_unknown_event_type(self):
        """Test formatting unknown event type."""
        event = {
            "type": "custom",
            "event": "something",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {
                "key": "value",
            },
        }

        result = format_event_cli(event)

        assert "CUSTOM" in result
        assert "something" in result

    def test_format_invalid_timestamp(self):
        """Test handling invalid timestamp."""
        event = {
            "type": "agent",
            "event": "spawned",
            "timestamp": "invalid",
            "data": {"task_id": "test:1", "status": "starting"},
        }

        result = format_event_cli(event)

        # Should show placeholder time
        assert "??:??:??" in result

    def test_format_missing_timestamp(self):
        """Test handling missing timestamp."""
        event = {
            "type": "agent",
            "event": "spawned",
            "data": {"task_id": "test:1", "status": "starting"},
        }

        result = format_event_cli(event)

        assert "??:??:??" in result

    def test_format_empty_data(self):
        """Test formatting event with empty data."""
        event = {
            "type": "session",
            "event": "stopped",
            "timestamp": "2024-01-15T10:30:00+00:00",
            "data": {},
        }

        result = format_event_cli(event)

        assert "SESSION" in result
        assert "stopped" in result


class TestEventAggregator:
    """Tests for EventAggregator class."""

    def test_init(self):
        """Test EventAggregator initialization."""
        agg = EventAggregator()

        assert agg.sessions == {}
        assert agg.agents == {}
        assert agg.recent_output == {}
        assert agg.stats_history == []

    def test_process_session_started(self):
        """Test processing session started event."""
        agg = EventAggregator()

        event = {
            "type": "session",
            "event": "started",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"session_name": "test-session", "tag": "feature"},
        }

        state = agg.process_event(event)

        assert "test-session" in agg.sessions
        assert agg.sessions["test-session"]["tag"] == "feature"
        assert len(state["sessions"]) == 1

    def test_process_session_stopped(self):
        """Test processing session stopped event."""
        agg = EventAggregator()

        # Start session first
        agg.process_event({
            "type": "session",
            "event": "started",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"session_name": "test-session"},
        })

        # Stop session
        agg.process_event({
            "type": "session",
            "event": "stopped",
            "timestamp": "2024-01-15T10:05:00Z",
            "data": {"session_name": "test-session"},
        })

        assert "test-session" not in agg.sessions

    def test_process_agent_spawned(self):
        """Test processing agent spawned event."""
        agg = EventAggregator()

        event = {
            "type": "agent",
            "event": "spawned",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"task_id": "test:1", "status": "starting"},
        }

        state = agg.process_event(event)

        assert "test:1" in agg.agents
        assert agg.agents["test:1"]["status"] == "starting"
        assert len(state["agents"]) == 1

    def test_process_agent_status_changed(self):
        """Test processing agent status changed event."""
        agg = EventAggregator()

        # Spawn agent
        agg.process_event({
            "type": "agent",
            "event": "spawned",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"task_id": "test:1", "status": "starting"},
        })

        # Change status
        agg.process_event({
            "type": "agent",
            "event": "status_changed",
            "timestamp": "2024-01-15T10:01:00Z",
            "data": {"task_id": "test:1", "status": "running"},
        })

        assert agg.agents["test:1"]["status"] == "running"

    def test_process_agent_status_changed_without_spawn(self):
        """Test processing status changed for unknown agent."""
        agg = EventAggregator()

        # Status change without spawn
        event = {
            "type": "agent",
            "event": "status_changed",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"task_id": "test:1", "status": "running"},
        }

        agg.process_event(event)

        # Should still track it
        assert "test:1" in agg.agents

    def test_process_output_lines(self):
        """Test processing output lines event."""
        agg = EventAggregator()

        event = {
            "type": "output",
            "event": "lines",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {
                "task_id": "test:1",
                "lines": ["line 1", "line 2"],
            },
        }

        agg.process_event(event)

        assert "test:1" in agg.recent_output
        assert agg.recent_output["test:1"] == ["line 1", "line 2"]

    def test_process_output_accumulates(self):
        """Test output lines accumulate."""
        agg = EventAggregator()

        # First batch
        agg.process_event({
            "type": "output",
            "event": "lines",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"task_id": "test:1", "lines": ["line 1", "line 2"]},
        })

        # Second batch
        agg.process_event({
            "type": "output",
            "event": "lines",
            "timestamp": "2024-01-15T10:00:01Z",
            "data": {"task_id": "test:1", "lines": ["line 3", "line 4"]},
        })

        assert agg.recent_output["test:1"] == ["line 1", "line 2", "line 3", "line 4"]

    def test_process_output_truncates_at_100(self):
        """Test output is truncated to last 100 lines."""
        agg = EventAggregator()

        # Add 150 lines
        for i in range(15):
            agg.process_event({
                "type": "output",
                "event": "lines",
                "timestamp": "2024-01-15T10:00:00Z",
                "data": {
                    "task_id": "test:1",
                    "lines": [f"line {j}" for j in range(i * 10, (i + 1) * 10)],
                },
            })

        # Should only have last 100
        assert len(agg.recent_output["test:1"]) == 100
        # First lines should be truncated
        assert "line 0" not in agg.recent_output["test:1"]
        # Last lines should be present
        assert "line 149" in agg.recent_output["test:1"]

    def test_process_stats(self):
        """Test processing stats event."""
        agg = EventAggregator()

        event = {
            "type": "stats",
            "event": "update",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {
                "session_name": "test-session",
                "running": 2,
                "completed": 1,
            },
        }

        state = agg.process_event(event)

        assert len(agg.stats_history) == 1
        assert state["stats"] == event["data"]

    def test_process_stats_history_limited(self):
        """Test stats history is limited to 100 entries."""
        agg = EventAggregator()

        # Add 150 stats updates
        for i in range(150):
            agg.process_event({
                "type": "stats",
                "event": "update",
                "timestamp": f"2024-01-15T10:{i:02d}:00Z",
                "data": {"count": i},
            })

        assert len(agg.stats_history) == 100
        # Should have last 100
        assert agg.stats_history[0]["count"] == 50
        assert agg.stats_history[-1]["count"] == 149

    def test_get_state(self):
        """Test get_state returns current aggregated state."""
        agg = EventAggregator()

        # Add some data
        agg.process_event({
            "type": "session",
            "event": "started",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"session_name": "test-session"},
        })
        agg.process_event({
            "type": "agent",
            "event": "spawned",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"task_id": "test:1", "status": "running"},
        })
        agg.process_event({
            "type": "stats",
            "event": "update",
            "timestamp": "2024-01-15T10:00:00Z",
            "data": {"running": 1},
        })

        state = agg.get_state()

        assert "sessions" in state
        assert "agents" in state
        assert "stats" in state
        assert len(state["sessions"]) == 1
        assert len(state["agents"]) == 1
        assert state["stats"]["running"] == 1

    def test_get_state_empty(self):
        """Test get_state with no data."""
        agg = EventAggregator()

        state = agg.get_state()

        assert state["sessions"] == []
        assert state["agents"] == []
        assert state["stats"] is None

    def test_multiple_sessions(self):
        """Test tracking multiple sessions."""
        agg = EventAggregator()

        for i in range(3):
            agg.process_event({
                "type": "session",
                "event": "started",
                "timestamp": "2024-01-15T10:00:00Z",
                "data": {"session_name": f"session-{i}"},
            })

        assert len(agg.sessions) == 3
        state = agg.get_state()
        assert len(state["sessions"]) == 3

    def test_multiple_agents(self):
        """Test tracking multiple agents."""
        agg = EventAggregator()

        for i in range(5):
            agg.process_event({
                "type": "agent",
                "event": "spawned",
                "timestamp": "2024-01-15T10:00:00Z",
                "data": {"task_id": f"task:{i}", "status": "running"},
            })

        assert len(agg.agents) == 5
        state = agg.get_state()
        assert len(state["agents"]) == 5
