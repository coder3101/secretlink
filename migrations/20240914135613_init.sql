-- Add migration script here
CREATE TABLE secrets (
	id SERIAL PRIMARY KEY,
	uuid UUID NOT NULL UNIQUE,
	secret TEXT NOT NULL,
	expiry INT CHECK (expiry >= 0),
	expired BOOLEAN DEFAULT FALSE,
	created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
	CONSTRAINT idx_uuid UNIQUE (uuid)
)
