-- Add down migration script here
ALTER TABLE provider_resource DROP COLUMN tokens;
ALTER TABLE provider_resource ALTER COLUMN priority type integer;
ALTER TABLE scrape ALTER COLUMN priority type integer;

DROP TABLE amqp_source;