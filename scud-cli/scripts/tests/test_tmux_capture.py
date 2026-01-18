"""
Unit tests for TmuxOutputCapture component.
"""

import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

sys.path.insert(0, str(Path(__file__).parent.parent))

from feed_publisher import TmuxOutputCapture


class TestTmuxOutputCapture:
    """Tests for the TmuxOutputCapture class."""

    def test_init(self):
        """Test TmuxOutputCapture initialization."""
        capture = TmuxOutputCapture(session_name="test-session", max_lines=100)

        assert capture.session_name == "test-session"
        assert capture.max_lines == 100
        assert capture._agent_states == {}

    def test_init_default_max_lines(self):
        """Test TmuxOutputCapture with default max_lines."""
        capture = TmuxOutputCapture(session_name="test-session")

        assert capture.max_lines == 100

    @patch("subprocess.run")
    def test_get_tmux_windows_success(self, mock_run):
        """Test getting tmux window list successfully."""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="task-test:1\ntask-test:2\ncontrol\n",
        )

        capture = TmuxOutputCapture(session_name="test-session")
        windows = capture.get_tmux_windows()

        assert windows == ["task-test:1", "task-test:2", "control"]
        mock_run.assert_called_once()
        call_args = mock_run.call_args
        assert "list-windows" in call_args[0][0]
        assert "-t" in call_args[0][0]
        assert "test-session" in call_args[0][0]

    @patch("subprocess.run")
    def test_get_tmux_windows_empty(self, mock_run):
        """Test getting empty window list."""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="",
        )

        capture = TmuxOutputCapture(session_name="test-session")
        windows = capture.get_tmux_windows()

        assert windows == []

    @patch("subprocess.run")
    def test_get_tmux_windows_failure(self, mock_run):
        """Test handling tmux command failure."""
        mock_run.return_value = MagicMock(
            returncode=1,
            stdout="",
            stderr="session not found",
        )

        capture = TmuxOutputCapture(session_name="test-session")
        windows = capture.get_tmux_windows()

        assert windows == []

    @patch("subprocess.run")
    def test_get_tmux_windows_timeout(self, mock_run):
        """Test handling subprocess timeout."""
        import subprocess

        mock_run.side_effect = subprocess.TimeoutExpired("tmux", 5)

        capture = TmuxOutputCapture(session_name="test-session")
        windows = capture.get_tmux_windows()

        assert windows == []

    @patch("subprocess.run")
    def test_get_tmux_windows_not_found(self, mock_run):
        """Test handling tmux not found."""
        mock_run.side_effect = FileNotFoundError("tmux not found")

        capture = TmuxOutputCapture(session_name="test-session")
        windows = capture.get_tmux_windows()

        assert windows == []

    @patch("subprocess.run")
    def test_capture_window_success(self, mock_run, sample_output_lines):
        """Test capturing window output successfully."""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="\n".join(sample_output_lines) + "\n\n\n",  # Trailing empty
        )

        capture = TmuxOutputCapture(session_name="test-session")
        lines = capture.capture_window("task-test:1")

        assert lines is not None
        assert len(lines) == len(sample_output_lines)
        assert lines[0] == "$ scud next"
        assert lines[-1] == "Task completed successfully!"

        # Verify subprocess call
        call_args = mock_run.call_args
        assert "capture-pane" in call_args[0][0]
        assert "-t" in call_args[0][0]
        assert "test-session:task-test:1" in call_args[0][0]

    @patch("subprocess.run")
    def test_capture_window_strips_trailing_empty(self, mock_run):
        """Test that trailing empty lines are stripped."""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="line1\nline2\n\n\n\n",
        )

        capture = TmuxOutputCapture(session_name="test-session")
        lines = capture.capture_window("task-test:1")

        assert lines == ["line1", "line2"]

    @patch("subprocess.run")
    def test_capture_window_failure(self, mock_run):
        """Test handling capture failure."""
        mock_run.return_value = MagicMock(
            returncode=1,
            stdout="",
        )

        capture = TmuxOutputCapture(session_name="test-session")
        lines = capture.capture_window("task-test:1")

        assert lines is None

    @patch("subprocess.run")
    def test_capture_window_timeout(self, mock_run):
        """Test handling capture timeout."""
        import subprocess

        mock_run.side_effect = subprocess.TimeoutExpired("tmux", 5)

        capture = TmuxOutputCapture(session_name="test-session")
        lines = capture.capture_window("task-test:1")

        assert lines is None

    @patch("subprocess.run")
    def test_get_output_diff_first_capture(self, mock_run, sample_output_lines):
        """Test first capture returns all lines."""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="\n".join(sample_output_lines) + "\n",
        )

        capture = TmuxOutputCapture(session_name="test-session")
        result = capture.get_output_diff("test:1", "task-test:1")

        assert result is not None
        new_lines, sequence = result
        assert new_lines == sample_output_lines
        assert sequence == 1

    @patch("subprocess.run")
    def test_get_output_diff_no_change(self, mock_run, sample_output_lines):
        """Test no diff when output hasn't changed."""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="\n".join(sample_output_lines) + "\n",
        )

        capture = TmuxOutputCapture(session_name="test-session")

        # First capture
        result1 = capture.get_output_diff("test:1", "task-test:1")
        assert result1 is not None

        # Second capture (same content)
        result2 = capture.get_output_diff("test:1", "task-test:1")
        assert result2 is None

    @patch("subprocess.run")
    def test_get_output_diff_new_lines(self, mock_run, sample_output_lines):
        """Test detecting new lines appended."""
        capture = TmuxOutputCapture(session_name="test-session")

        # First capture with initial lines
        initial_lines = sample_output_lines[:5]
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="\n".join(initial_lines) + "\n",
        )

        result1 = capture.get_output_diff("test:1", "task-test:1")
        assert result1 is not None
        assert result1[1] == 1  # sequence 1

        # Second capture with more lines
        all_lines = sample_output_lines
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="\n".join(all_lines) + "\n",
        )

        result2 = capture.get_output_diff("test:1", "task-test:1")

        assert result2 is not None
        new_lines, sequence = result2
        # Should return the new lines
        assert sequence == 2
        # New lines should be from the end
        assert len(new_lines) > 0

    @patch("subprocess.run")
    def test_get_output_diff_sequence_increments(self, mock_run):
        """Test that sequence number increments with each change."""
        capture = TmuxOutputCapture(session_name="test-session")

        for i in range(5):
            mock_run.return_value = MagicMock(
                returncode=0,
                stdout=f"line {i}\n",
            )

            result = capture.get_output_diff("test:1", "task-test:1")

            assert result is not None
            _, sequence = result
            assert sequence == i + 1

    @patch("subprocess.run")
    def test_get_output_diff_multiple_agents(self, mock_run):
        """Test tracking multiple agents independently."""
        capture = TmuxOutputCapture(session_name="test-session")

        # Agent 1
        mock_run.return_value = MagicMock(returncode=0, stdout="agent1 line\n")
        result1 = capture.get_output_diff("test:1", "task-test:1")
        assert result1 is not None
        assert result1[1] == 1

        # Agent 2
        mock_run.return_value = MagicMock(returncode=0, stdout="agent2 line\n")
        result2 = capture.get_output_diff("test:2", "task-test:2")
        assert result2 is not None
        assert result2[1] == 1  # Independent sequence

        # Agent 1 again
        mock_run.return_value = MagicMock(returncode=0, stdout="agent1 line\nagent1 line2\n")
        result3 = capture.get_output_diff("test:1", "task-test:1")
        assert result3 is not None
        assert result3[1] == 2  # Continues from agent 1's sequence

    @patch("subprocess.run")
    def test_get_output_diff_capture_failure(self, mock_run):
        """Test handling capture failure in diff."""
        mock_run.return_value = MagicMock(returncode=1, stdout="")

        capture = TmuxOutputCapture(session_name="test-session")
        result = capture.get_output_diff("test:1", "task-test:1")

        assert result is None

    @patch("subprocess.run")
    def test_get_output_diff_creates_state(self, mock_run):
        """Test that get_output_diff creates agent state."""
        mock_run.return_value = MagicMock(returncode=0, stdout="test\n")

        capture = TmuxOutputCapture(session_name="test-session")

        assert "test:1" not in capture._agent_states

        capture.get_output_diff("test:1", "task-test:1")

        assert "test:1" in capture._agent_states
        state = capture._agent_states["test:1"]
        assert state.task_id == "test:1"
        assert state.window_name == "task-test:1"
        assert state.sequence == 1

    @patch("subprocess.run")
    def test_get_output_diff_with_ansi_codes(self, mock_run):
        """Test handling ANSI color codes in output."""
        colored_output = "\x1b[32mSuccess\x1b[0m: Task completed\n\x1b[31mError\x1b[0m: Warning\n"
        mock_run.return_value = MagicMock(returncode=0, stdout=colored_output)

        capture = TmuxOutputCapture(session_name="test-session")
        result = capture.get_output_diff("test:1", "task-test:1")

        assert result is not None
        new_lines, _ = result
        # ANSI codes should be preserved (let consumer handle stripping)
        assert "\x1b[32m" in new_lines[0]
