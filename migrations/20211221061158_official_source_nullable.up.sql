-- Add up migration script here
ALTER TABLE provider_resource ALTER COLUMN official SET NOT NULL;