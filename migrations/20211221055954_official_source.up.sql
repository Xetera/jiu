-- Add up migration script here
ALTER TABLE provider_resource ADD COLUMN official boolean default false;