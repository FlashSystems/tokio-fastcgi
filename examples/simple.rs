use tokio::net::TcpListener;
use tokio_fastcgi::{Requests, RequestResult};

#[tokio::main]
async fn main() {
	let addr = "127.0.0.1:8080";
	let listener = TcpListener::bind(addr).await.unwrap();

	loop {
		let connection = listener.accept().await;
		// Accept new connections
		match connection {
			Err(err) => {
				println!("Establishing connection failed: {}", err);
				break;
			},
			Ok((mut stream, address)) => {
				println!("Connection from {}", address);

				// If the socket connection was established successfully spawn a new task to handle
				// the requests that the webserver will send us.
				tokio::spawn(async move {
					// Create a new requests handler it will collect the requests from the server and
					// supply a streaming interface.
					let mut requests = Requests::from_split_socket(stream.split(), 10, 10);

					// Loop over the requests via the next method and process them.
					while let Ok(Some(request)) = requests.next().await {
						if let Err(err) = request.process(|_request| async move {
							// This is the place to handle the FastCGI request and return a result.
							RequestResult::Complete(0)
						}).await {
							// This is the error handler that is called if the process call returns an error.
							println!("Processing request failed: {}", err);
						}
					}
				});
			}
		}
	}
}
