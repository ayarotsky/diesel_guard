-- Test migration with safe operations
ALTER TABLE users ADD COLUMN email VARCHAR(255);
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
