-- Safe: Index with 3 columns
CREATE INDEX CONCURRENTLY idx_users_composite ON users(tenant_id, user_id, email);
