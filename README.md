![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)
[![Current Version](https://img.shields.io/crates/v/tokio-fastcgi)](https://crates.io/crates/tokio-fastcgi)
[![Docs.rs](https://docs.rs/tokio-fastcgi/badge.svg)](https://docs.rs/tokio-fastcgi)
![License Apache 2.0](https://img.shields.io/crates/l/tokio-fastcgi)

# Async FastCGI handler library

This crate implements a FastCGI handler for Tokio. It's a complete re-implementation of the FastCGI protocol in safe rust and supports all three FastCGI roles: Responder, Authorizer and Filter. The [`Role`] enum documents the different roles and their input and output parameters.

If you just want to use this library, look at the examples, open the documentation and start using it by adding the following to the `[dependencies]` section of your `Cargo.toml`:

```toml
tokio-fastcgi = "1.0"
```

## Principle of operation

The tokio-fastcgi library handles FastCGI requests that are sent by a server. Accepting the connection and spawning a new task to handle the requests is done via Tokio. Within the handler task, [`Requests::from_split_socket`] is called to create an asynchronous requests stream. Calling [`Requests::next().await`](Requests::next) on this stream will return a new [`Request`] instance as soon as it was completely received from the web-server.

The returned [`Request`] instance has a [`process`](Request::process) method that accepts an asynchronous callback function or closure that will process the request. The current [request](Request) will be passed to the callback as a parameter and can be used to retrieve the input streams sent by the web-server and to write to the output streams. The callback returns a [result](RequestResult) or an error, if processing the request failed.

This is repeated while the [`Requests`] instance for the connection returns more requests. If no more requests are returned, the stream will be dropped and the connection to the web-server will be closed.

This library handles connection reuse and aborting requests for the user. See [`Requests::next`] for more details.
