"""
Unit tests for SessionWatcher component.
"""

import asyncio
import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent.parent))

from feed_publisher import SessionWatcher


class TestSessionWatcher:
    """Tests for the SessionWatcher class."""

    def test_init(self, scud_dir: Path):
        """Test SessionWatcher initialization."""
        watcher = SessionWatcher(scud_dir, callback=None)

        assert watcher.scud_dir == scud_dir
        assert watcher.spawn_dir == scud_dir / "spawn"
        assert watcher._last_states == {}

    @pytest.mark.asyncio
    async def test_check_sessions_empty_dir(self, scud_dir: Path):
        """Test checking sessions when spawn directory is empty."""
        watcher = SessionWatcher(scud_dir, callback=None)

        changes = await watcher.check_sessions()

        assert changes == []

    @pytest.mark.asyncio
    async def test_check_sessions_no_spawn_dir(self, temp_dir: Path):
        """Test checking sessions when spawn directory doesn't exist."""
        scud_dir = temp_dir / ".scud"
        scud_dir.mkdir()
        # Don't create spawn directory

        watcher = SessionWatcher(scud_dir, callback=None)
        changes = await watcher.check_sessions()

        assert changes == []

    @pytest.mark.asyncio
    async def test_check_sessions_new_session(
        self,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
    ):
        """Test detecting a new session file."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write session file
        write_session_file("test-session", sample_session_data)

        # Check for changes
        changes = await watcher.check_sessions()

        assert len(changes) == 1
        change_type, session_name, session_data = changes[0]
        assert change_type == "new"
        assert session_name == "test-session"
        assert session_data["session_name"] == "test-session"
        assert len(session_data["agents"]) == 2

    @pytest.mark.asyncio
    async def test_check_sessions_no_change(
        self,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
    ):
        """Test that unchanged sessions are not reported."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write session file
        write_session_file("test-session", sample_session_data)

        # First check - should detect new
        changes1 = await watcher.check_sessions()
        assert len(changes1) == 1
        assert changes1[0][0] == "new"

        # Second check - should detect nothing (no change)
        changes2 = await watcher.check_sessions()
        assert len(changes2) == 0

    @pytest.mark.asyncio
    async def test_check_sessions_changed(
        self,
        scud_dir: Path,
        sample_session_data: dict,
        sample_session_data_updated: dict,
        write_session_file,
    ):
        """Test detecting a changed session file."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write initial session file
        write_session_file("test-session", sample_session_data)

        # First check
        await watcher.check_sessions()

        # Update session file
        write_session_file("test-session", sample_session_data_updated)

        # Second check - should detect change
        changes = await watcher.check_sessions()

        assert len(changes) == 1
        change_type, session_name, session_data = changes[0]
        assert change_type == "changed"
        assert session_name == "test-session"
        assert len(session_data["agents"]) == 3  # Updated data has 3 agents

    @pytest.mark.asyncio
    async def test_check_sessions_removed(
        self,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
    ):
        """Test detecting a removed session file."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write session file
        session_path = write_session_file("test-session", sample_session_data)

        # First check
        await watcher.check_sessions()

        # Remove session file
        session_path.unlink()

        # Second check - should detect removal
        changes = await watcher.check_sessions()

        assert len(changes) == 1
        change_type, session_name, session_data = changes[0]
        assert change_type == "removed"
        assert session_name == "test-session"
        assert session_data is None

    @pytest.mark.asyncio
    async def test_check_sessions_multiple(
        self,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
    ):
        """Test detecting multiple new sessions."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write multiple session files
        for i in range(3):
            data = sample_session_data.copy()
            data["session_name"] = f"session-{i}"
            write_session_file(f"session-{i}", data)

        # Check for changes
        changes = await watcher.check_sessions()

        assert len(changes) == 3
        session_names = {c[1] for c in changes}
        assert session_names == {"session-0", "session-1", "session-2"}
        assert all(c[0] == "new" for c in changes)

    @pytest.mark.asyncio
    async def test_check_sessions_mixed_changes(
        self,
        scud_dir: Path,
        sample_session_data: dict,
        sample_session_data_updated: dict,
        write_session_file,
    ):
        """Test detecting mixed changes (new, changed, removed)."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Initial setup: two sessions
        write_session_file("session-1", sample_session_data)
        path2 = write_session_file("session-2", sample_session_data)

        # First check
        await watcher.check_sessions()

        # Make changes:
        # - session-1: modify
        # - session-2: remove
        # - session-3: add new
        write_session_file("session-1", sample_session_data_updated)
        path2.unlink()
        write_session_file("session-3", sample_session_data)

        # Check for changes
        changes = await watcher.check_sessions()

        assert len(changes) == 3

        changes_dict = {c[1]: c[0] for c in changes}
        assert changes_dict["session-1"] == "changed"
        assert changes_dict["session-2"] == "removed"
        assert changes_dict["session-3"] == "new"

    @pytest.mark.asyncio
    async def test_check_sessions_invalid_json(
        self,
        scud_dir: Path,
    ):
        """Test handling of invalid JSON files."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write invalid JSON
        invalid_path = scud_dir / "spawn" / "invalid.json"
        invalid_path.write_text("{ invalid json }")

        # Should not raise, should return empty (file skipped)
        changes = await watcher.check_sessions()

        assert len(changes) == 0

    @pytest.mark.asyncio
    async def test_check_sessions_non_json_files_ignored(
        self,
        scud_dir: Path,
        sample_session_data: dict,
    ):
        """Test that non-JSON files are ignored."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write a non-JSON file
        (scud_dir / "spawn" / "README.md").write_text("# Readme")
        (scud_dir / "spawn" / "backup.bak").write_text("{}")

        # Check for changes
        changes = await watcher.check_sessions()

        assert len(changes) == 0

    @pytest.mark.asyncio
    async def test_check_sessions_state_persistence(
        self,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
    ):
        """Test that watcher maintains state across multiple checks."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write session
        write_session_file("test-session", sample_session_data)

        # Multiple checks should only report new once
        changes1 = await watcher.check_sessions()
        changes2 = await watcher.check_sessions()
        changes3 = await watcher.check_sessions()

        assert len(changes1) == 1
        assert len(changes2) == 0
        assert len(changes3) == 0

        # State should be tracked
        assert "test-session" in watcher._last_states

    @pytest.mark.asyncio
    async def test_check_sessions_whitespace_only_change(
        self,
        scud_dir: Path,
        sample_session_data: dict,
        write_session_file,
    ):
        """Test that whitespace-only changes are detected (different hash)."""
        watcher = SessionWatcher(scud_dir, callback=None)

        # Write session with compact JSON
        path = scud_dir / "spawn" / "test-session.json"
        path.write_text(json.dumps(sample_session_data))

        # First check
        await watcher.check_sessions()

        # Rewrite with indentation (different bytes, same data)
        path.write_text(json.dumps(sample_session_data, indent=4))

        # Should detect as change (hash differs)
        changes = await watcher.check_sessions()
        assert len(changes) == 1
        assert changes[0][0] == "changed"
