-- Add up migration script here
ALTER TABLE scrape ADD COLUMN IF NOT EXISTS scraped_at TIMESTAMP WITHOUT TIME ZONE DEFAULT NOW();
