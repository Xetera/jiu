-- Add up migration script here
INSERT INTO amqp_source (provider_name, provider_destination, metadata)
SELECT name, destination, '{}' from provider_resource
ON CONFLICT DO NOTHING;
