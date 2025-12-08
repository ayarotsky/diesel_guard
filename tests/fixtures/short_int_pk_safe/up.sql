-- Safe: BIGINT and BIGSERIAL primary keys
-- These avoid ID exhaustion

-- BIGINT primary key
CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    name TEXT
);

-- BIGSERIAL primary key (auto-incrementing BIGINT)
CREATE TABLE posts (
    id BIGSERIAL PRIMARY KEY,
    title TEXT
);

-- Composite PK with all BIGINT columns
CREATE TABLE events (
    tenant_id BIGINT,
    event_id BIGINT,
    PRIMARY KEY (tenant_id, event_id)
);

-- INT is safe for non-PK columns
CREATE TABLE lookups (
    id BIGINT PRIMARY KEY,
    code INT UNIQUE,
    name TEXT
);
