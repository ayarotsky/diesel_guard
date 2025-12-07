-- Example of unsafe ADD UNIQUE constraint via ALTER TABLE
-- This acquires ACCESS EXCLUSIVE lock, blocking all reads and writes

ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);
