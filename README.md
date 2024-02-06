![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)
[![Current Version](https://img.shields.io/crates/v/tokio-fastcgi)](https://crates.io/crates/tokio-fastcgi)
[![Docs.rs](https://docs.rs/tokio-fastcgi/badge.svg)](https://docs.rs/tokio-fastcgi)
![License Apache 2.0](https://img.shields.io/crates/l/tokio-fastcgi)

# Async FastCGI handler library

This crate implements a FastCGI handler for Tokio. It's a complete re-implementation of the FastCGI protocol in safe rust and supports all three FastCGI roles: Responder, Authorizer and Filter. The [`Role`](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/enum.Role.html) enum documents the different roles and their input and output parameters.

If you just want to use this library, look at the examples, open the documentation and start using it by adding the following to the `[dependencies]` section of your `Cargo.toml`:

```toml
tokio-fastcgi = "1"
```

## Principle of operation

The tokio-fastcgi library handles FastCGI requests that are sent by a server. Accepting the connection and spawning a new task to handle the requests is done via Tokio. Within the handler task, [`Requests::from_split_socket`](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Requests.html#method.from_split_socket) is called to create an asynchronous requests stream. Calling [`Requests::next().await`](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Requests.html#method.next) on this stream will return a new [`Request`](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Request.html) instance as soon as it was completely received from the web-server.

The returned [`Request`](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Request.html) instance has a [`process`](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Request.html#method.process) method that accepts an asynchronous callback function or closure that will process the request. The current [request](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Request.html) will be passed to the callback as a parameter and can be used to retrieve the input streams sent by the web-server and to write to the output streams. The callback returns a [result](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/enum.RequestResult.html) or an error, if processing the request failed.

This is repeated while the [`Requests`](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Requests.html) instance for the connection returns more requests. If no more requests are returned, the stream will be dropped and the connection to the web-server will be closed.

This library handles connection reuse and aborting requests for the user. See [`Requests::next`](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Requests.html#method.next) for more details.

## Examples

The library contains two examples: [A bare bones one](https://github.com/FlashSystems/tokio-fastcgi/blob/master/examples/simple.rs) and a litte [REST API](https://github.com/FlashSystems/tokio-fastcgi/blob/master/examples/apiserver.rs). Just have a look :)

## Changelog

* Version 1.0.0\
  Initial release

* Version 1.1.0\
  Add methods to enumerate the parameters of a request ([params_iter](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Request.html#method.params_iter) and [str_params_iter](https://docs.rs/tokio-fastcgi/latest/tokio_fastcgi/struct.Request.html#method.str_params_iter)).

* Version 1.1.1\
  Update dependency versions. Make dependency to `once_cell` less restrictive.

* Version 1.2.0\
  Fix bug #4: Under heavy load, FastCGI responses are not delivered correctly. This makes the FastCGI protocol fail and connections get dropped with various error messages. This release fixes this problem. The `tokio-fastcgi` library is now stable even under heavy load.
