
<h1>
  <img src="https://i.imgur.com/qVp1N9y.png">
</h1>

<p align="center">
  <b>Scrape multiple media providers on a cron job and dispatch webhooks when changes are detected.</b>
</p>

## Jiu
Jiu is a multi-threaded media scraper capable of juggling thousands of endpoints from different providers with unique restrictions/requirements.


## Providers

Provider is the umbrella term that encapsulates all endpoints for a given domain.

For example, https://weverse.io/bts/artist and https://weverse.io/dreamcatcher/artist are 2 endpoints under the Weverse provider.

### Supported providers

* [Pinterest Boards](https://www.pinterest.com/janairaoliveira314/handong)
* [Weverse.io](https://weverse.io/dreamcatcher/feed)


## Priority

Priority is the system that determines how frequently an endpoint needs to be queued to be checked again on a scale from 1 to 10.  Priority **1** endpoints are checked once every 2 hours and priority **10** endpoints are checked once a week.

All new endpoints start with a priority of **5** and move up or down based on how frequently changes are being detected. Endpoints that don't yield changes frequently are moved down one level and every detected change moves the endpoint up one level.

## Rate Limits
Each provider has a rate limit shared across the same domain to prevent bans. This can be customized per-provider to allow for higher or stricter rate limits or bursts sizes based on what the API allows.
## Authorization

Anonymous request are always preferred when possible.

There is a customizable login flow for providers that require authorization which allows logging into APIs after an authorization error, and persists additional data (such as a JWT token) to be shared across each provider during the lifetime of the process.

The login flow is reversed engineered for providers that don't have a public API.

> Juggling multiple accounts per provider is currently not supported and probably won't be as long as long as your accounts aren't getting banned (and if they are then you're sending too many requests and need to increase your rate limits).

Jiu will try its best to identify itself in its requests' `User-Agent` header, but will submit a fake UA for providers that gate posts behind a user agent check (currently none).

## Proxies
Proxies are not supported or needed.

## Webhooks

Jiu is capable of sending webhooks to multiple destinations when an update for a provider is detected.
```json
{
  "provider": {
    "type": "weverse.artist_feed",
    "id": "14",
    "page": "https://weverse.io/dreamcatcher/artist",
  },
  "media": [
    {
      "type": "image",
      "media_url": "https://cdn-contents-web.weverse.io/user/xlx2048/jpg/76b26c9cb3a543698893d410cb244a01973.jpg",
      "page_url": "https://weverse.io/dreamcatcher/artist/1666332291423293?photoId=217518938",
      "post_date": "2021-07-26T05:33:48Z",
      "reference_url": "https://weverse.io/dreamcatcher/artist/1666332291423293?photoId=217518938",
      "unique_identifier": "217518938",
      "provider_metadata": {
        "post_id": 241523,
        "author_id": 63,
        "author_name": "시연",
        "height": 2048,
        "width": 3072,
        "thumbnail_url": "https://cdn-contents-web.weverse.io/user/mx750/jpg/76b26c9cb3a543698893d410cb244a01973.jpg"
      }
    }
  ]
},
```

Every provider has its own `provider_metadata` field that _may_ contain extra information about the image or the post it was found under, but may also be missing. _Documentation WIP_

The `unique_identifier` field is unique **per provider** and not globally.

If a Discord webhook URL is detected, the payload is changed to allow Discord to display the images in the channel.

There is currently no retry mechanism for webhooks that fail to deliver successfully.

## Jiu is **NOT**:
* For bombarding sites like Twitter with requests to detect changes within seconds.
* Capable of executing javascript with a headless browser.
* Able to send requests to any social media site without explicit support.

## Jiu **IS**:
* For slowly monitoring changes in different feeds over the course of multiple hours without abusing the provider.
* Capable of adjusting the frequency of scrapes based on how frequently the source is updated.
* Able to send webhooks to different sites like Discord for automatic updates.
* The lead singer of [Dreamcatcher](https://www.youtube.com/watch?v=1QD0FeZyDtQ).

## Usage

Copy over `.env.example` to `.env` and fill out relevant fields

WIP

> If you would like to use this project, please change the `USER_AGENT` environment variable to identify your crawler accurately.

Built for [simp.pics](https://github.com/xetera/simp.pics)
