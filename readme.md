# Spinitron API Relay Server
## Overview
When run on a server and linked with a radio station's Spinitron, this server will provide a REST API for accessing the station's Spinitron data without exposing a private API key. Once up and running, you can make GET requests to the server to get the station's current spins, shows, and DJ info. 

This server caches the data it gets from Spinitron, which is useful for applications that need to make frequent requests, such as a station website. It uses Spinitron's metadata push feature to make minimal requests while keeping spin data current. Additionally, this relay server provides a server-sent event (SSE) endpoint that can inform web clients when a new spin is logged instead of having to continually poll the server for new spins.

This project is built with Rust and Warp. It's stable and quite efficient. It's been load-tested up to 20k requests per second with no issues, which should be enough for most stations.

This was originally built as a project for KSCU 103.3 FM out of Santa Clara University, check us out [here](https://kscu.org).

## Endpoints

| Endpoint | Details |
| :--- | :--- |
| `spins/get` | Returns the ten most recent tracks logged in Spinitron.
| `spins/stream` | An [SSE](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events) stream that clients can connect to. Will send the message `Spin outdated - Update needed.` when new data should be fetched. Can be used with `spins/get` to keep spin data updated.
| `spins/update` | Forces relay server to fetch new spin data from Spinitron. Must contain an Request Header `Content Type` of `application/x-www-form-urlencoded`.
| `shows/get` | Returns either the current show and next upcoming show or, if no show is live, next two upcoming shows.
| `shows/update` | Forces relay server to fetch new show data from Spinitron. Must contain an Request Header `Content Type` of `application/x-www-form-urlencoded`.

## Local Installation

1. Install Rust and Cargo. You can find instructions [here](https://www.rust-lang.org/tools/install).

2. Clone this repository and navigate to it.
```
git clone https://github.com/aidansmth/API_relay.git
cd API_relay
```

3. Find your Spinitron API key by logging in as an administrator and navigating to `Admin > Automation and API`. Add the the API key as an environmental variable to your current shell instance with `export SPIN_KEY=your_api_key`.

4. From the root of the project, run the program with `LOCAL=OK cargo run`. With 'LOCAL=OK', endpoints will be avaliable at `localhost:8080` instead of port 80. _Spins will not update automatically unless run externally and configured with Spinitron below._

## Running in a Container
This project can be run using either Docker directly or Docker Compose. Choose the method that best suits your needs.

### Option 1: Using Docker

1. Build the Docker image:
   ```
   docker build -t api_relay .
   ```

2. Run the container:
   ```
   docker run -d --name {CHOOSE CONTAINER NAME} -p 80:80 -e SPIN_KEY="{SPINITRON API KEY}" --restart unless-stopped api_relay
   ```

   Replace `your_spin_key_here` with your actual SPIN_KEY.

3. Access the application at `http://localhost`

### Option 2: Using Docker Compose

1. Make sure you have Docker Compose installed on your system.

2. Either add SPIN_KEY as an environmental variable or create a `.env` file in the project root and add your SPIN_KEY:
   ```
   SPIN_KEY={SPINITRON API KEY}
   ```

3. Run the following command to start the service:
   ```
   docker-compose up
   ```

4. Access the application at `http://localhost`

## Configuring HTTPS with AWS
HTTPS may be a security requirement if browsers are sending requests to the Relay, such as for a (station website)[kscu.org]. Given the high costs of AWS load balancers, I now recommend using a single cloud instance running both a container and nginx reverse proxy to handle HTTPS certificates. Instructions on how to do this can be found (here)[].

## Limitations
- `/spins/get` only returns the last ten logged spins.
- `/shows/get` returns either the current show and next upcoming show or, if no show is live, next two upcoming shows.
- Show info is only updated every fifteen minutes at minute 0, 15, 30, & 45 of each hour. If you update a live or upcoming show within Spinitron, use the `/shows/update` endpoint to force the server to update.
- As show info is fetched at the top of the hour, it can take a second or two to update on the server. It's safe to fetch new show data three seconds after the top of the hour.

## Dependencies

- [**Tokio**](https://docs.rs/tokio/1/tokio/) - A runtime for asynchronous programming in Rust, enabling non-blocking I/O operations. We're using the full feature set.

- [**Tokio-cron-scheduler**](https://docs.rs/tokio-cron-scheduler/0.9.4/tokio_cron_scheduler/) - A cron-like scheduler for tokio. It's used to schedule tasks to run at specific times.

- [**Warp**](https://docs.rs/warp/0.3/warp/) - A super-fast, composable, streaming web server framework. 

- [**Serde**](https://docs.rs/serde/1.0/serde/) - A framework for serializing and deserializing Rust data structures. The "derive" feature is enabled for automatic trait implementation.

- [**Serde Derive**](https://docs.rs/serde_derive/1.0.152/serde_derive/) - Provides the `#[derive(Deserialize, Serialize)]` macros for serde.

- [**Serde JSON**](https://docs.rs/serde_json/1.0/serde_json/) - A JSON library for Rust, built on serde.

- [**Reqwest**](https://docs.rs/reqwest/0.11/reqwest/) - An easy and powerful Rust HTTP Client. We use the "blocking" and "json" features for synchronous requests and JSON support respectively.

- [**Pretty Env Logger**](https://docs.rs/pretty_env_logger/0.4/pretty_env_logger/) - A logger configured via an environment variable, giving pretty output.

- [**Log**](https://docs.rs/log/0.4/log/) - A flexible logging library for Rust.

- [**Chrono**](https://docs.rs/chrono/0.4.23/chrono/) - A date and time library for Rust.

- [**Futures-util**](https://docs.rs/futures-util/0.3.27/futures_util/) - A library providing utilities for working with futures and streams.

- [**Tokio-stream**](https://docs.rs/tokio-stream/0.1.12/tokio_stream/) - Provides Stream types for working with Tokio.

- [**Log4rs**](https://docs.rs/log4rs/1.2.0/log4rs/) - A highly configurable logging framework modeled after Java's Logback and log4j libraries.

## Issues

If you run into any issues, I'm happy to help. Please reach out by creating an issue on GitHub.