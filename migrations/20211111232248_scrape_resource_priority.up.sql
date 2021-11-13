-- Add up migration script here
ALTER TABLE provider_resource ADD COLUMN last_token_update TIMESTAMP WITHOUT TIME ZONE NULL;
ALTER TABLE provider_resource ADD COLUMN tokens DECIMAL NOT NULL DEFAULT 1.0;
ALTER TABLE provider_resource ALTER COLUMN priority type decimal;
ALTER TABLE scrape ALTER COLUMN priority type decimal NOT NULL DEFAULT 1.0;
