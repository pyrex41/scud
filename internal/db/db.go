package db

import (
	"database/sql"
	"fmt"
	"path/filepath"
	"time"

	_ "modernc.org/sqlite"
)

// DB wraps an SQLite database for transcript storage.
type DB struct {
	db *sql.DB
}

// Open opens (or creates) the transcript database in the given scud directory.
func Open(scudDir string) (*DB, error) {
	dbPath := filepath.Join(scudDir, "transcripts.db")
	conn, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("opening database: %w", err)
	}
	// WAL mode for better concurrent access
	if _, err := conn.Exec("PRAGMA journal_mode=WAL"); err != nil {
		conn.Close()
		return nil, fmt.Errorf("setting WAL mode: %w", err)
	}
	d := &DB{db: conn}
	if err := d.migrate(); err != nil {
		conn.Close()
		return nil, err
	}
	return d, nil
}

// Close closes the database connection.
func (d *DB) Close() error { return d.db.Close() }

// migrate applies schema migrations if needed.
func (d *DB) migrate() error {
	// Check current version. If migrations table doesn't exist, we need full schema.
	var version int
	row := d.db.QueryRow("SELECT COALESCE(MAX(version), 0) FROM migrations")
	if err := row.Scan(&version); err != nil {
		// Table doesn't exist yet, create everything
		if _, err := d.db.Exec(schemaSQL); err != nil {
			return fmt.Errorf("creating schema: %w", err)
		}
		_, err = d.db.Exec("INSERT INTO migrations (version, applied_at) VALUES (?, ?)",
			currentVersion, time.Now().UTC().Format(time.RFC3339))
		return err
	}
	// Already up to date
	return nil
}
