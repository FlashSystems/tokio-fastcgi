use std::{sync::Arc, collections::HashMap, io::Read};

use tokio::{net::{TcpListener, tcp::OwnedWriteHalf}, sync::RwLock};
use tokio_fastcgi::{Requests, RequestResult, Request};

// This is a little example of a REST API server implemeted in FastCGI.
//
// To include it into your Apache setup you can use the `proxy_fcgi` module.
// The following configuration will pass all requests to the path `/api` to the
// server application listening on port 8080.
//
// ``` conf
// <Location /api>
//   ProxyPass "fcgi://127.0.0.1:8080/" enablereuse=on
// </Location>
// ```
//
// To try out this example, configure Apache accordingly and start the example
// by running `cargo run --example apiserver`.
//
// Now you can use curl to interact with it:
//
// # Adding a quote
//
// To add a quote execute `curl -X POST --fail-with-body -d 'My quote' http://localhost/api/quote`.
// This will return the number of the quote. If no quotes are already there it
// will return `1`.
//
// # Retrieving a quote
//
// After adding a quote you can execute `curl --fail-with-body http://localhost/api/quote/1` to
// fetch the quote. If the quote does not exist a 404 error will be returned.
//
// # Update a quote
//
// Quotes can be updated by PUTing a new one. To try this out execute
// `curl -X PUT --fail-with-body -d 'My updated quote' http://localhost/api/quote/1`.
// If you try to PUT a non existent quote, it will be created with the given id.
// You can check that the new quote was saved by GETing it.
//
// # Deleting a quote
//
// To delete a quote the DELTE HTTP method is used. A quote can be deleted
// by calling `curl -X DELETE --fail-with-body http://localhost/api/quote/1`.
// The DELETE request will return the deleted quote.
//
// # Having a second path
//
// To demonstrate how multiple paths can be handled, there is a `/api/ping` path
// that only knows the `GET` method and will alwys return the string `pong`.

/// Define some response codes to use.
struct HttpResponse {
	code: u16,
	message: &'static str
}

impl HttpResponse {
	fn ok() -> Self {
		Self { code: 200, message: "Ok" }
	}

	fn e400() -> Self {
		Self { code: 400, message: "Bad Request" }
	}

	fn e404() -> Self {
		Self { code: 404, message: "Not Found" }
	}

	fn e405() -> Self {
		Self { code: 405, message: "Method Not Allowed" }
	}

	fn e500() -> Self {
		Self { code: 500, message: "Internal Server Error" }
	}
}

/// Request handler trait. All request handlers have to implement this async trait.
/// The default implementation for every method returns the 405 (Method Not Allowed) error code.
trait RequestHandler {
	async fn get(_store: Arc<RwLock<Store>>, _request: &Request<OwnedWriteHalf>, _selector: Option<u32>) -> Result<String, HttpResponse> {
		Err(HttpResponse::e405())
	}

	async fn put(_store: Arc<RwLock<Store>>, _request: &Request<OwnedWriteHalf>, _selector: Option<u32>) -> Result<String, HttpResponse> {
		Err(HttpResponse::e405())
	}

	async fn post(_store: Arc<RwLock<Store>>, _request: &Request<OwnedWriteHalf>, _selector: Option<u32>) -> Result<String, HttpResponse> {
		Err(HttpResponse::e405())
	}

	async fn delete(_store: Arc<RwLock<Store>>, _request: &Request<OwnedWriteHalf>, _selector: Option<u32>) -> Result<String, HttpResponse> {
		Err(HttpResponse::e405())
	}
}

/// A simple data store implementation using a HashMap.
struct Store {
	pub quotes: HashMap<u32, String>
}

impl Store {
	fn new() -> Self {
		Self {
			quotes: HashMap::default()
		}
	}
}

/// Handles the REST API for quotes.
struct Quotes {}

impl RequestHandler for Quotes {
	/// Get returns the quote stored for the given selector u32 or 404 (Not Found).
	async fn get(store: Arc<RwLock<Store>>, _request: &Request<OwnedWriteHalf>, selector: Option<u32>) -> Result<String, HttpResponse> {
		if let Some(selector) = selector {
			if let Some(quote) = store.read().await.quotes.get(&selector) {
				Ok(quote.clone())
			} else {
				Err(HttpResponse::e404())
			}
		} else {
			Err(HttpResponse::e404())
		}
	}

	/// Put puts a quote into the given selector.
	/// If the selector is missing we return 405 (Method Not Allowed) because you can not call PUT on the root resource.
	async fn put(store: Arc<RwLock<Store>>, request: &Request<OwnedWriteHalf>, selector: Option<u32>) -> Result<String, HttpResponse> {
		if let Some(selector) = selector {
			let mut quote = String::default();

			if request.get_stdin().read_to_string(&mut quote).is_ok() {
				store.write().await.quotes.insert(selector, quote);

				Ok("".to_string())
			} else {
				Err(HttpResponse::e500())
			}
		} else {
			Err(HttpResponse::e405())
		}
	}

	/// Post findes the next free selector u32 and puts the quote there.
	/// It returns the selector where the quote was stored.
	async fn post(store: Arc<RwLock<Store>>, request: &Request<OwnedWriteHalf>, selector: Option<u32>) -> Result<String, HttpResponse> {
		// A post can only be done on the root resource not with a selector.
		// If a selector was passed return 405 - Method Not Allowed
		if selector.is_some() {
			return Err(HttpResponse::e405());
		}

		let mut quote = String::default();

		if request.get_stdin().read_to_string(&mut quote).is_ok() {
			let mut store = store.write().await;

			let next_free_selector = store.quotes.keys().max().unwrap_or(&0) + 1;
			store.quotes.insert(next_free_selector, quote);

			Ok(next_free_selector.to_string())
		} else {
			Err(HttpResponse::e500())
		}
	}

	/// Delete removes the quote described by the selector u32.
	/// If no selector is passed we return 405 (Method Not Allowed) because the root resource can not be deleted.
	async fn delete(store: Arc<RwLock<Store>>, _request: &Request<OwnedWriteHalf>, selector: Option<u32>) -> Result<String, HttpResponse> {
		if let Some(selector) = selector {
			if let Some(quote) = store.write().await.quotes.remove(&selector) {
				Ok(quote)
			} else {
				Err(HttpResponse::e404())
			}
		} else {
			Err(HttpResponse::e405())
		}
	}
}

/// Handle ping requests on /api/ping
struct Ping {}

impl RequestHandler for Ping {
	async fn get(_store: Arc<RwLock<Store>>, _request: &Request<OwnedWriteHalf>, _selector: Option<u32>) -> Result<String, HttpResponse> {
		Ok("pong".to_string())
	}
}

/// Encodes the HTTP status code and the response string and sends it back to the webserver.
async fn send_response(request: Arc<Request<OwnedWriteHalf>>, response_code: HttpResponse, data: Option<&str>) -> Result<RequestResult, tokio_fastcgi::Error> {
	request.get_stdout().write(format!("Status: {} {}\n\n", response_code.code, response_code.message).as_bytes()).await?;
	if let Some(data) = data {
		request.get_stdout().write(data.as_bytes()).await?;
	}

	Ok(RequestResult::Complete(0))
}

/// Calles the appropriate method handler on a request handler.
async fn method_handler<H: RequestHandler>(store: Arc<RwLock<Store>>, request: &Request<OwnedWriteHalf>, selector: Option<u32>) -> Result<String, HttpResponse> {
	let method = request.get_str_param("request_method");

	println!("Calling method {} on endpoint", method.unwrap_or_default());

	match method
	{
		Some("GET") => H::get(store, request, selector).await,
		Some("PUT") => H::put(store, request, selector).await,
		Some("POST") => H::post(store, request, selector).await,
		Some("DELETE") => H::delete(store, request, selector).await,
		_ => Err(HttpResponse::e405())
	}
}

/// Dispatcher that uses the endpoint name extracted from the URI path component to call the matching method handler.
/// Every call is done via the generic function `method_hanlder` that does the dispatching of the method to a function
/// implemented by the `RequestHandler` trait.
async fn process_endpoint(store: Arc<RwLock<Store>>, request: Arc<Request<OwnedWriteHalf>>, endpoint: &str, selector: Option<&str>) -> Result<RequestResult, tokio_fastcgi::Error> {

	println!("Processing endpoint '{}' with selector '{}'", endpoint, selector.unwrap_or_default());

	let result = match endpoint {
		"quote" => method_handler::<Quotes>(store, &request, selector.map(|s| s.parse().unwrap() )).await,
		"ping" => method_handler::<Ping>(store, &request, selector.map(|s| s.parse().unwrap() )).await,
		_ => Err(HttpResponse::e404())
	};

	match result {
		Ok(result) => send_response(request, HttpResponse::ok(), Some(&result)).await,
		Err(http_response) => send_response(request, http_response, None).await
	}
}

/// Called by the `main` function if a new request as arrived via FastCGI. This function parses the request,
/// extracts the URI path component and calls `process_endpoint` passing the different path elements.
async fn process_request(store: Arc<RwLock<Store>>, request: Arc<Request<OwnedWriteHalf>>) -> Result<RequestResult, tokio_fastcgi::Error> {
	// Check that a `request_uri` parameter was passed by the webserver. If this is not the case,
	// fail with a HTTP 400 (Bad Request) error code.
	if let Some(request_uri) = request.get_str_param("request_uri").map(String::from) {
		// Split the request URI into the different path componets.
		// The following match is used to extract and verify the path compontens.
		let mut request_parts = request_uri.split_terminator('/').fuse();
		match (request_parts.next(), request_parts.next(), request_parts.next(), request_parts.next(), request_parts.next()) {
			// Process /api/<endpoint>[/<selector>]
			(Some(""), Some("api"), Some(endpoint), selector, None) => process_endpoint(store, request, endpoint, selector).await,

			// Verything else will return HTTP 404 (Not Found)
			_ => send_response(request, HttpResponse::e404(), None).await
		}
	} else {
		send_response(request, HttpResponse::e400(), None).await
	}
}

#[tokio::main]
async fn main() {
	let addr = "127.0.0.1:8080";

	let listener = TcpListener::bind(addr).await.unwrap();

	let store = Arc::new(RwLock::new(Store::new()));

	loop {
		let connection = listener.accept().await;
		// Accept new connections
		match connection {
			Err(err) => {
				println!("Establishing connection failed: {err}");
				break;
			},
			Ok((stream, address)) => {
				println!("Connection from {address}");

				let conn_store = store.clone();

				// If the socket connection was established successfully spawn a new task to handle
				// the requests that the webserver will send us.
				tokio::spawn(async move {
					// Create a new requests handler it will collect the requests from the server and
					// supply a streaming interface.
					let mut requests = Requests::from_split_socket(stream.into_split(), 10, 10);

					// Loop over the requests via the next method and process them.
					while let Ok(Some(request)) = requests.next().await {
						let req_store = conn_store.clone();

						if let Err(err) = request.process(|request| async move {
							process_request(req_store.clone(), request).await.unwrap()
						}).await {
							// This is the error handler that is called if the process call returns an error.
							println!("Processing request failed: {err}");
						}
					}
				});
			}
		}
	}
}
