-- Your SQL goes here
CREATE TABLE IF NOT EXISTS webhook(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  destination TEXT NOT NULL,
  created_at TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
  -- extra data attached to a webhook invocation
  metadata JSONB
);

CREATE TABLE IF NOT EXISTS provider_resource(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  -- This can be a FQDN or an identifier that maps to a unique API endpoint
  -- on the provider's end
  destination TEXT,
  name TEXT,
  priority INTEGER NOT NULL DEFAULT 5 CHECK(priority >= 1 AND priority <= 10),
  UNIQUE(destination, name)
);

CREATE TABLE IF NOT EXISTS scrape(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  provider_resource_id INTEGER NOT NULL REFERENCES provider_resource(id)
);

-- each scrape can have more than one request
CREATE TABLE IF NOT EXISTS scrape_request(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  scrape_id INTEGER REFERENCES scrape(id),
  date TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
  page INTEGER NOT NULL DEFAULT 1,
  response_code INTEGER,
  -- how long did the response take in ms
  response_delay INTEGER,
  scraped_at TIMESTAMP WITHOUT TIME ZONE NOT NULL
);

CREATE TABLE IF NOT EXISTS media(
  -- This is necessary when trying to sort media that were
  -- crawled at the same time
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  provider_resource_id INTEGER REFERENCES provider_resource(id) ON DELETE SET NULL,
  scrape_request_id INTEGER REFERENCES scrape_request(id) ON DELETE SET NULL,
  -- We are assuming there is only one type of url
  url TEXT NOT NULL UNIQUE,
  -- a unique identifier that's specific to the provider
  unique_identifier TEXT NOT NULL,
  -- could be null if the provider doesn't have the information
  posted_at TIMESTAMP WITHOUT TIME ZONE,
  discovered_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
  UNIQUE(unique_identifier, provider_resource_id)
  -- we don't want to crawl the same data multiple times
);

CREATE TABLE IF NOT EXISTS scrape_error(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  -- already declared in scrape_request
  -- response_code INTEGER,
  response_body TEXT NOT NULL DEFAULT '',
  scrape_request_id INTEGER NOT NULL REFERENCES scrape_request(id)
);

CREATE TABLE IF NOT EXISTS webhook_sources( 
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  webhook_id INTEGER REFERENCES webhook(id),
  provider_resource_id INTEGER REFERENCES provider_resource(id)
);

CREATE TABLE IF NOT EXISTS webhook_invocation(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  scrape_id INTEGER /* NOT NULL */ REFERENCES scrape(id),
  webhook_id INTEGER /* NOT NULL */ REFERENCES webhook(id) ON DELETE SET NULL,
  response_code INTEGER,
  response_delay INTEGER,
  invoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- SELECT create_hypertable('media', 'added_at');
-- SELECT create_hypertable('scrape_request', 'scraped_at');