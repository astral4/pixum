# Pixum
Pixum is a Pixiv artwork API built with [Axum](https://github.com/tokio-rs/axum), a Rust web framework.
[Redis](https://redis.io/) is used for caching image URLs. The application is containerized with [Docker](https://www.docker.com/) and [Docker Compose](https://docs.docker.com/compose/).

## API reference
- `/`: Introduction page
- `/:id`: Gets information about the work with content ID `id`
- `/:id/:num`: Gets the image at content ID `id` on page number `num`

## Quickstart
To run the app locally:
1. Make sure Docker Compose is installed
2. Run `docker compose up` (or `docker-compose up` if using Compose standalone)
3. Go to `localhost:3000`

## Acknowledgments
This project was inspired by [pixiv.cat](https://github.com/pixiv-cat/pixivcat-cloudflare-workers).