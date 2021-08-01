-- Add up migration script here
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
  destination TEXT NOT NULL,
  name TEXT NOT NULL,
  enabled BOOLEAN DEFAULT True,
  -- the url for the scraped page
  url TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 5 CHECK(priority >= 1 AND priority <= 10),
  last_scrape TIMESTAMP WITHOUT TIME ZONE NULL,
  -- the date last scrape was requested, this acts a lock to prevent resources from being accessed multiple times 
  last_queue TIMESTAMP WITHOUT TIME ZONE NULL,
  created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT NOW()
  UNIQUE(destination, name)
);

CREATE TABLE IF NOT EXISTS scrape(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  provider_name TEXT,
  provider_destination TEXT,
  -- the priority this scrape was executed against
  priority NOT NULL CHECK(priority >= 1 AND priority <= 10)
  FOREIGN KEY (provider_name, provider_destination)
    REFERENCES provider_resource(name, destination) ON DELETE SET NULL ON UPDATE CASCADE
);

-- each scrape can have more than one request
CREATE TABLE IF NOT EXISTS scrape_request(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  scrape_id INTEGER REFERENCES scrape(id),
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
  provider_name TEXT,
  provider_destination TEXT,
  scrape_request_id INTEGER REFERENCES scrape_request(id) ON DELETE SET NULL,
  -- We are assuming there is only one type of url
  image_url TEXT NOT NULL UNIQUE,
  page_url TEXT NULL,
  reference_url TEXT NULL,
  -- a unique identifier that's specific to the provider
  unique_identifier TEXT NOT NULL,
  -- where the image is coming from
  -- could be null if the provider doesn't have the information
  posted_at TIMESTAMP WITHOUT TIME ZONE NULL,
  discovered_at TIMESTAMP WITHOUT TIME ZONE NOT NULL,
  UNIQUE(unique_identifier, provider_name),
  FOREIGN KEY (provider_name, provider_destination)
    REFERENCES provider_resource(name, destination) ON UPDATE CASCADE ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS scrape_error(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  -- already declared in scrape_request
  -- response_code INTEGER,
  response_body TEXT NOT NULL DEFAULT '',
  response_code TEXT NOT NULL,
  message TEXT NULL,
  scrape_id INTEGER NOT NULL REFERENCES scrape(id)
);

CREATE TABLE IF NOT EXISTS webhook_source(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  webhook_id INTEGER REFERENCES webhook(id),
  provider_name TEXT,
  provider_destination TEXT,
  FOREIGN KEY (provider_name, provider_destination)
    REFERENCES provider_resource(name, destination) ON DELETE SET NULL ON UPDATE CASCADE
);

CREATE TABLE IF NOT EXISTS webhook_invocation(
  id INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  scrape_id INTEGER /* NOT NULL */ REFERENCES scrape(id),
  webhook_id INTEGER /* NOT NULL */ REFERENCES webhook(id) ON DELETE SET NULL,
  response_code INTEGER,
  response_delay INTEGER,
  invoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ON scrape (provider_destination, provider_name);
