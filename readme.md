# Spinitron API Relay Server
## Overview
When run on a server and linked with a radio station's Spinitron, this server will provide a REST API for accessing the station's Spinitron data without exposing a private API key. Once up and running, you can make GET requests to the server to get the station's current spins, shows, and DJ info. 

This server caches the data it gets from Spinitron, which is useful for applications that need to make frequent requests, such as a station website. Additionally, the server provides a server-sent event (SSE) endpoint `/spins/stream/` that can inform clients when a new spin is logged instead of clients having to continually poll the server for new spins.

This project is built with Rust and Warp. It pretty stable and relatively efficient. It's been load-tested up to 20k requests per second with no issues, which should be enough for most small stations.

This was originally built as a student project for KSCU 103.3 FM out of Santa Clara University, check us out [here](https://kscu.org).

## Installation
1. Install Rust and Cargo. You can find instructions [here](https://www.rust-lang.org/tools/install).

2. Clone this repository and navigate to it.
```
git clone https://github.com/aidansmth/API_relay.git
cd API_relay
```

3. Create and edit a `.env` file in the root directory of the project. This file will contain your station's Spinitron API key.
```
SPINITRON_API_KEY=your_api_key
```

## Running Locally
From the root of the project, run the file using the line below. _Please note that spins will not update automatically unless run externally and configured with Spinitron below._
```
LOCAL=OK cargo run
```

## Running on a Server
The following instructions create an instance of the API Relay Server hosted on AWS. For other hosts or to host the server yourself, different containerization schemes may apply.

1. Create a `.env` file in the root directory of the project. This file will contain your station's Spinitron API key. _If you did this earlier to run project locally, you can skip this step._

2. Create a deployable .zip file. From the root of your project, run the following command. You should see a repo.zip file created in /target/.
```
git archive --add-file .env --format=zip HEAD -o ./target/repo.zip
```

3. To launch the server on AWS's Elastic Beanstalk service, navigate to the Elastic Beanstalk page within AWS and then click `Create environment`. Then, use the following configuration options. If a configuration option is not listed, it shouldn't been needed for the application to function correctly so use your best judgement. 

| Field | Recommended Setting |
| :--- | :--- |
| Environment tier | Web server environment |
| Platform type | Managed Platform |
| Platform | Docker |
| Platform branch | Docker running on 64bit Amazon Linux 2 |
| Platform version | 3.5.9 |
| Application code | Upload your code |
| Local file | Choose the repo.zip file you created earlier |
| Preset | Single instance (free tier eligible) |

4. Continue with configuration

| Field | Recommended Setting |
| :--- | :--- |
| Service role | Create and use new service role |
| Public IP address | Activated |
| Health reporting | Enchanced |

5. To launch your instance, click `Submit` after reviewing your settings. Please note that it can take up to twenty minutes for Elastic Beanstalk to have your instance be fully accessible.

6. <details><summary>Configuring Spinitron Metadata Push</summary> <br>

    Your API server will rely on Spinitron's Metadata  Push feature to update the server with new tracks as they're detected/logged to Spinitron. This prevents constant polling of Spinitron's API by the server.

    1. Login in to Spinitron as an administrator and navigate to `Admin > Metadata Push`. 
    2. Click `New Channel` and enter the following. You can leave all other fields with their defaults.

    | Field | Recommended Setting |
    | :--- | :--- |
    | Channel Name | API Relay Push |
    | Template | POST http://[insert your server's web address]/spins/update
    | Enabled | True |

    3. To ensure that the Metadata Push is working successfully, navigate to `Metadata Push Logs` and wait for a new spin to be detected/logged. If successful, you should see a push with the status code `OK`.
</details>

7. Once launched and the health status is listed as healthly, you can access your server using it's web address (should resemble `*.[aws-region].elasticbeanstalk.com/`). The easiest endpoint to test is `http://[your server's web address]/spins/get`.

## Configuring HTTPS and Routing
This is recommended for advanced users only, but is likely needed if you're making  a program here requests to the API Relay are made client side, such as a station website.

1. Route a custom domain or subdomain through AWS Route 53.

2. Provision an HTTPS certificate for that custom domain using AWS Certificate Manager.

3. Create a load balancer in AWS Elastic Beanstalk. Be sure to add the HTTPS Certificate from the last step to the load balancer.

4. Configure HTTP and HTTPS forwarding rules. Your load balancer should accept both HTTP and HTTPS, but only forward all requests to your Elastic Beanstalk instance as HTTP.

5. Test by requesting spin data using HTTPS, such as `https://[your custom domain]/spins/get`

## Limitations
- `/spins/get` only returns the last ten logged spins.
- `/shows/get` returns either the current show and next upcoming show or, if no show is live, two upcoming shows.
- Show info is only updated every fifteen minutes at minute 0, 15, 30, & 45 of each hour. If you update a live or upcoming show within Spinitron, use the `/shows/update` endpoint to force the server to update.
- As show info is fetched at the top of the hour, it can take a second or two to update on the server. It's safe to fetch new show data three seconds after the top of the hour.

## Dependencies

This project uses several dependencies to provide its functionality. You'll need the following Rust crates:

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