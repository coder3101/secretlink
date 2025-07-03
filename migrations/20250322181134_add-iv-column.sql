-- Add migration script here
ALTER TABLE secrets
ADD COLUMN iv TEXT;
