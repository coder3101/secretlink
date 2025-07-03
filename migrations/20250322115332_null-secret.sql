-- Add migration script here
ALTER TABLE secrets
  ALTER COLUMN secret DROP NOT NULL;
