-- Add down migration script here

ALTER TABLE amqp_source DROP CONSTRAINT amqp_unique_providers;
