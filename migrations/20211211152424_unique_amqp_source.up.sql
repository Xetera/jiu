-- Add up migration script here
ALTER TABLE amqp_source ADD CONSTRAINT amqp_unique_providers UNIQUE (provider_name, provider_destination);
