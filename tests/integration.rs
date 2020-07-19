//! This source file runs the tests defined in `commons.rs` against the
//! FastCGI implementation in `lib.rs`. It creates a mockup input and output
//! stream and connects it to the FastCGI implementation in `lib.rs`.
//!
//! The tests are declared within commons.rs because they are the same as
//! the server tests. That way a test can be used directly on the API
//! and via the network to properly test the FastCGI implementation.
mod commons;
use tokio_fastcgi::Requests;

use crate::commons::*;

pub async fn run_test<T: TestCase>() {
	let mut requests = Requests::new(T::get_input(), T::get_output(), 5, 10);
	while let Some(request) = requests.next().await.expect("Request could not be constructed.") {
		request.process(T::processor).await.expect("Error while processing.");
	}
}

#[tokio::test]
async fn params_in_out() {
	run_test::<TestParamsInOut>().await;
}

#[tokio::test]
async fn role_authorizer() {
	run_test::<TestRoleAuthorizer>().await;
}

#[tokio::test]
async fn role_filter() {
	run_test::<TestRoleFilter>().await;
}

#[tokio::test]
async fn abort_request() {
	run_test::<TestAbortRequest>().await;
}

#[tokio::test]
async fn abort_continue() {
	run_test::<TestAbortContinue>().await;
}

#[tokio::test]
async fn test_unknown_role_return() {
	run_test::<TestUnknownRoleReturn>().await;
}

#[tokio::test]
#[should_panic(expected = "InvalidRoleNumber")]
async fn test_unknown_role_request() {
	run_test::<TestUnknownRoleRequest>().await;
}

#[tokio::test]
async fn unkown_request_type() {
	run_test::<TestUnknownRequestType>().await;
}

#[tokio::test]
async fn get_values() {
	run_test::<TestGetValues>().await;
}

#[tokio::test]
async fn keep_connection() {
	run_test::<TestKeepConnection>().await;
}