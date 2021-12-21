-- Add down migration script here
ALTER TABLE provider_resource ALTER COLUMN official DROP NOT NULL;
