//! This source file runs the tests defined in `commons.rs` against a real
//! Tokio network server at localhost.
//!
//! This tests if the tokio-fastcgi crate can really be used as a FastCGI
//! server.
//!
//! Tests that lead to a panic within the server process can not be used
//! with this test setup. They are only used via the integration tests
//! that can handle the panic correctly. The following tests are not run by
//! this test suite:
//!
//! - TestUnknownRoleRequest
//!
//! The tests are declared within commons.rs because they are the same as
//! the integration tests. That way a test can be used directly on the API
//! and via the network to properly test the FastCGI implementation.
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime;
use tokio_fastcgi::Requests;
use std::{io::{Read, Write}, panic};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, Shutdown, TcpStream};
use std::sync::{LazyLock, Mutex};
use std::sync::mpsc::sync_channel;

mod commons;
use crate::commons::*;

/// Global lock to prevent running more than one server via multithreaded tests.
static GLOBAL_LOCK: LazyLock<Mutex<u32>> = LazyLock::new(|| { Mutex::default() });

/// This is a test fixture that will run a FastCGI server on a free port and
/// connect to it to run some test cases.
fn network_test<T: TestCase>() {
	let rt = runtime::Builder::new_multi_thread()
		.enable_all()
		.build().unwrap();

	// A channel to signal the test client that the test server is ready.
	// This channel transports the local port used by the server.
	let (rdy_sender, rdy_receiver) = sync_channel(1);

	// A channel to singal the test client that the server has processed the request.
	let (done_sender, done_receiver) = sync_channel(1);

	rt.spawn(async move {
		let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).await.unwrap();

		// Build the server
		let server = async move {

			rdy_sender.send(listener.local_addr().unwrap().port()).unwrap();

			loop {
				let mut accept_count = 0;

				match listener.accept().await {
					Err(err) => {
						println!("Establishing connection failed: {}", err);
						break;
					},
					Ok(mut socket) => {
						accept_count += 1;

						// Create a clone of done_sender that can be owned by the async lambda.
						let done_sender = done_sender.clone();

						tokio::spawn(async move {
							let mut requests = Requests::from_split_socket(socket.0.split(), 5, 10);

							while let Some(request) = requests.next().await.expect("Request could not be constructed.") {
								request.process(T::processor).await.expect("Error while processing.");
							}

							// Tell the testbed that we're done.
							done_sender.send(accept_count).unwrap();
						});
					}
				}
			}
		};

		// Start the server and block this async fn until `server` spins down.
		server.await;
	});

	rt.block_on(async {
		let port = rdy_receiver.recv().unwrap();

		let mut s = TcpStream::connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)).unwrap();

		{
			let mut input = T::get_input();
			let mut buffer: Vec<u8> = Vec::new();
			input.read_to_end(&mut buffer).await.unwrap();
			s.write_all(&buffer).unwrap();
			s.flush().unwrap();
		}

		// Shut down the sending side. This makes it possible for the
		// server to exit his loop and send us a done message.
		s.shutdown(Shutdown::Write).unwrap();

		{
			let mut output = T::get_output();
			let mut buffer: Vec<u8> = Vec::new();
			loop {
				if let Ok(revc_count) = done_receiver.try_recv() {
					assert_eq!(revc_count, 1, "More than one connection was encountered.");
					break;
				}

				s.read_to_end(&mut buffer).unwrap();
				output.write_all(&buffer).await.unwrap();
			}
		}
	});
}

/// Wrapper that runs the network test and makes sure, that only one
/// test runs in parallel.
fn run_network_test<T: TestCase>() {
	// Make sure we only run this test once, even with mulithreaded tests.
	// Having more than one Tokio runtime within the test suite leads to all
	// kind of problems.
	// This mutex makes sure the server can only run once.
	let guard = GLOBAL_LOCK.lock().unwrap();

	// Catch panics and make sure that the lock is droped before the
	// panic is resumed and the thread is crashed. This prevents the lock
	// from being poisoned.
	let result = panic::catch_unwind(|| {
		network_test::<T>();
	});

	drop(guard);

	if let Err(err) = result {
		panic::resume_unwind(err);
	}
}

#[test]
fn params_in_out() {
	run_network_test::<TestParamsInOut>();
}

#[test]
fn role_authorizer() {
	run_network_test::<TestRoleAuthorizer>();
}

#[test]
fn role_filter() {
	run_network_test::<TestRoleFilter>();
}

#[test]
fn abort_request() {
	run_network_test::<TestAbortRequest>();
}

#[test]
fn abort_continue() {
	run_network_test::<TestAbortContinue>();
}

#[test]
fn test_unknown_role_return() {
	run_network_test::<TestUnknownRoleReturn>();
}

#[test]
fn unkown_request_type() {
	run_network_test::<TestUnknownRequestType>();
}
#[test]
fn get_values() {
	run_network_test::<TestGetValues>();
}

#[test]
fn keep_connection() {
	run_network_test::<TestKeepConnection>();
}
