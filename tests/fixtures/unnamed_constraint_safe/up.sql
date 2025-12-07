-- Example of properly named constraints in a migration
-- Note: UNIQUE constraints via ALTER TABLE are always unsafe (even when named)
-- For UNIQUE, use CREATE UNIQUE INDEX CONCURRENTLY instead (see add_unique_constraint_safe)

-- Named CHECK constraint (safe)
ALTER TABLE users ADD CONSTRAINT users_age_check CHECK (age >= 0);

-- Named FOREIGN KEY constraint (safe)
ALTER TABLE posts ADD CONSTRAINT posts_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id);
