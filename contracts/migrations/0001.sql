PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
BEGIN;
CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL) STRICT;
CREATE TABLE instances(
 id TEXT PRIMARY KEY, name TEXT NOT NULL, root_handle TEXT NOT NULL UNIQUE,
 spec_json TEXT NOT NULL CHECK(json_valid(spec_json)),
 active_revision TEXT,
 FOREIGN KEY(active_revision, id) REFERENCES revisions(id, instance_id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE TABLE revisions(
 id TEXT PRIMARY KEY, instance_id TEXT NOT NULL REFERENCES instances(id),
 parent_id TEXT, lock_json TEXT NOT NULL CHECK(json_valid(lock_json)), created_at TEXT NOT NULL,
 UNIQUE(id, instance_id),
 FOREIGN KEY(parent_id, instance_id) REFERENCES revisions(id, instance_id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
CREATE TABLE artifacts(
 sha256 TEXT PRIMARY KEY CHECK(length(sha256)=64), size_bytes INTEGER NOT NULL CHECK(size_bytes>=0),
 relative_path TEXT NOT NULL UNIQUE, upstream_hashes_json TEXT NOT NULL CHECK(json_valid(upstream_hashes_json)),
 verified_at TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('verified','quarantined'))
) STRICT;
CREATE TABLE revision_artifacts(
 revision_id TEXT NOT NULL REFERENCES revisions(id), target_path TEXT NOT NULL,
 sha256 TEXT NOT NULL REFERENCES artifacts(sha256), role TEXT NOT NULL,
 ownership TEXT NOT NULL CHECK(ownership IN ('pack','user')),
 PRIMARY KEY(revision_id, target_path)
) STRICT;
CREATE TABLE transactions(
 id TEXT PRIMARY KEY, instance_id TEXT NOT NULL REFERENCES instances(id),
 base_revision TEXT, next_revision TEXT NOT NULL, journal_path TEXT NOT NULL UNIQUE,
 state TEXT NOT NULL CHECK(state IN ('PREPARING','PREPARED','APPLYING','DB_COMMITTED','FINALIZED','ROLLED_BACK','RECOVERY_CONFLICT')),
 plan_json TEXT NOT NULL CHECK(json_valid(plan_json)), updated_at TEXT NOT NULL
) STRICT;
CREATE UNIQUE INDEX one_active_write ON transactions(instance_id)
 WHERE state IN ('PREPARING','PREPARED','APPLYING','DB_COMMITTED','RECOVERY_CONFLICT');
CREATE TABLE tasks(
 id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE, request_digest TEXT NOT NULL,
 instance_id TEXT REFERENCES instances(id), kind TEXT NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('queued','running','waiting_user','blocked_external','succeeded','failed','cancelled')),
 sequence INTEGER NOT NULL DEFAULT 1 CHECK(sequence>0), snapshot_json TEXT NOT NULL CHECK(json_valid(snapshot_json)),
 updated_at TEXT NOT NULL
) STRICT;
CREATE TABLE accounts(
 id TEXT PRIMARY KEY, player_uuid TEXT, display_name TEXT, credential_reference TEXT,
 status TEXT NOT NULL, public_json TEXT NOT NULL CHECK(json_valid(public_json))
) STRICT;
CREATE TABLE runtimes(
 id TEXT PRIMARY KEY, executable_path TEXT NOT NULL, major INTEGER NOT NULL CHECK(major>0),
 architecture TEXT NOT NULL, fingerprint TEXT NOT NULL, verified_at TEXT NOT NULL
) STRICT;
CREATE TABLE plugins(
 id TEXT PRIMARY KEY, publisher TEXT NOT NULL, version TEXT NOT NULL, manifest_digest TEXT NOT NULL,
 permissions_json TEXT NOT NULL CHECK(json_valid(permissions_json)), enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
 failure_count INTEGER NOT NULL DEFAULT 0 CHECK(failure_count>=0)
) STRICT;
CREATE TABLE settings(key TEXT PRIMARY KEY, value_json TEXT NOT NULL CHECK(json_valid(value_json))) STRICT;
INSERT INTO schema_migrations VALUES(1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
COMMIT;
