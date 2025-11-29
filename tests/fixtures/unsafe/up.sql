-- Test migration with unsafe operations
ALTER TABLE users ADD COLUMN admin BOOLEAN DEFAULT FALSE;
