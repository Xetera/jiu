-- Add up migration script here
ALTER TABLE provider_resource ADD COLUMN last_token_update TIMESTAMP WITHOUT TIME ZONE NULL;
ALTER TABLE provider_resource ADD COLUMN tokens DECIMAL NOT NULL DEFAULT 1.0;
ALTER TABLE provider_resource ALTER COLUMN priority type decimal;
ALTER TABLE scrape ALTER COLUMN priority type decimal NOT NULL DEFAULT 1.0;
ALTER TABLE provider_resource DROP CONSTRAINT provider_resource_priority_check;
ALTER TABLE scrape DROP CONSTRAINT scrape_priority_check;
ALTER TABLE webhook DROP COLUMN metadata;
ALTER TABLE webhook_source ADD COLUMN metadata JSONB;

CREATE TABLE IF NOT EXISTS amqp_source(
     id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
     provider_name TEXT,
     provider_destination TEXT,
     metadata JSONB,
     FOREIGN KEY (provider_name, provider_destination)
         REFERENCES provider_resource(name, destination) ON DELETE SET NULL ON UPDATE CASCADE
);
