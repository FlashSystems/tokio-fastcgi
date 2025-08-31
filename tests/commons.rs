//! This source file contains tests that verify the implementation of the
//! FastCGI protocol.
//!
//! The tests are used as integration tests within `integration.rs` and as
//! real tests over the network on `server.ts`.
//!
//! The tests implement the data structures of the FastCGI protocol without
//! using the structures and enums from `lib.rs` to make sure errors are not
//! canceled out by the same error within the test suite.
use tokio_fastcgi::{Request, RequestResult, Role};
use tokio_test::io::{Builder, Mock};
use std::sync::Arc;
use std::time::Duration;
use std::io::Read;
use std::convert::From;
use std::future::Future;
use tokio::io::AsyncWrite;

pub enum RecordType {
	BeginRequest = 1,
	AbortRequest = 2,
	EndRequest = 3,
	Params = 4,
	StdIn = 5,
	StdOut = 6,
	StdErr = 7,
	Data = 8,
	GetValues = 9,
	GetValuesResult = 10,
	UnkownType = 11,
	InvalidType = 99
}

pub enum RecordRole {
	Responder = 1,
	Authorizer = 2,
	Filter = 3
}

pub enum RecordFlags {
	KeepConn = 1
}

/// Create a fastcgi-record for testing.
pub fn create_record(request_type: RecordType, request_id: u8, padding: u8, data: &[u8]) -> Vec<u8> {
	let content_length = data.len() as u16;

	let mut record = vec![0x01, request_type as u8, 0x00, request_id, (content_length >> 8 & 0xFF) as u8, (content_length & 0xFF) as u8, padding, 0x00];

	record.extend_from_slice(data);
	record.extend_from_slice(&*vec![0u8; padding as usize]);

	record
}

pub trait TestCase {
	fn get_input() -> Mock;
	fn get_output() -> Mock;
	fn processor<W: AsyncWrite + Unpin + Send>(request: Arc<Request<W>>) -> impl Future<Output = RequestResult> + Send;
}

pub struct TestParamsInOut {}

impl TestCase for TestParamsInOut {
	fn get_input() -> Mock {
		Builder::new()
			.read(&create_record(RecordType::BeginRequest, 0x01, 0x09, &[ 0x00, RecordRole::Responder as u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]))
			.read(&create_record(RecordType::Params, 0x01, 0x06, b"\x0B\x02SERVER_PORT80\x04\x03TESTYES\x06\x03NOUTF8NO\xF0"))
			.read(&create_record(RecordType::Params, 0x01, 0x00, &[]))
			.read(&create_record(RecordType::StdIn, 0x01, 0x03, &(0..100u8).collect::<Vec<u8>>()[..] ))	// Fill StdIn
			.read(&create_record(RecordType::StdIn, 0x01, 0x03, &[]))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 1, 0, 8, 0, 0, b'T', b'E', b'S', b'T', b'1', b'2', b'3', b'4', 1, 6, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 1, 0, 8, 0, 0, 222, 173, 190, 239, 0, 0, 0, 0])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(request: Arc<Request<W>>) -> RequestResult {
		// Check the parameters
		let sp = request.get_param("SERVER_PORT");
		assert!(sp.is_some());
		assert_eq!(sp.unwrap(), &[b'8', b'0']);
		let sp = request.get_str_param("SERVER_PORT");
		assert!(sp.is_some());
		assert_eq!(sp.unwrap(), "80");

		let tst = request.get_param("TEST");
		assert!(tst.is_some());
		assert_eq!(tst.unwrap(), &[b'Y', b'E', b'S']);
		let tst = request.get_str_param("TEST");
		assert!(tst.is_some());
		assert_eq!(tst.unwrap(), "YES");

		let noutf8 = request.get_param("NOUTF8");
		assert!(noutf8.is_some());
		assert_eq!(noutf8.unwrap(), &[b'N', b'O', 0xF0]);
		assert!(request.get_str_param("NOUTF8").is_none());

		assert!(request.get_param("SERVER_DUMMY").is_none());

		// Test the params iterator
		let mut params: Vec<(&str, &[u8])> = request.params_iter().unwrap().collect();
		assert_eq!(params.len(), 3);
		params.sort();
		assert_eq!(params[0].0, "noutf8");
		assert_eq!(params[0].1, &[b'N', b'O', 0xF0]);
		assert_eq!(params[1].0, "server_port");
		assert_eq!(params[1].1, &[b'8', b'0']);
		assert_eq!(params[2].0, "test");
		assert_eq!(params[2].1, &[b'Y', b'E', b'S']);

		// Test the string params iterator
		let mut params: Vec<(&str, Option<&str>)> = request.str_params_iter().unwrap().collect();
		assert_eq!(params.len(), 3);
		params.sort();
		assert_eq!(params[0].0, "noutf8");
		assert_eq!(params[0].1, None);
		assert_eq!(params[1].0, "server_port");
		assert_eq!(params[1].1, Some("80"));
		assert_eq!(params[2].0, "test");
		assert_eq!(params[2].1, Some("YES"));

		// Check if stdin is valid
		let mut stdin = [0u8; 100];
		assert!(request.get_stdin().read_exact(&mut stdin).is_ok());
		for (i, x) in stdin.iter().enumerate() {
			assert_eq!(*x, i as u8);
		}

		// Write some Output to verify StdOut
		request.get_stdout().write(b"TEST1234").await.unwrap();
		RequestResult::Complete(0xDEADBEEF)
	}
}

pub struct TestRoleAuthorizer {}

impl TestCase for TestRoleAuthorizer {
	fn get_input() -> Mock {
		Builder::new()
			.read(&create_record(RecordType::BeginRequest, 0x01, 0x00, &[ 0x00, RecordRole::Authorizer as u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]))
			.read(&create_record(RecordType::Params, 0x01, 0x06, b"\x04\x02USERME"))
			.read(&create_record(RecordType::Params, 0x01, 0x00, &[]))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 1, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(request: Arc<Request<W>>) -> RequestResult {
		assert_eq!(request.role, Role::Authorizer);

		// Check the parameters
		let user = request.get_param("USER");
		assert!(user.is_some());
		assert_eq!(String::from_utf8(user.unwrap().to_vec()).unwrap(), "ME");

		// Write some Output to verify StdOut
		RequestResult::Complete(0x00)
	}
}

pub struct TestRoleFilter {}

impl TestCase for TestRoleFilter {
	fn get_input() -> Mock {
		Builder::new()
			.read(&create_record(RecordType::BeginRequest, 0x01, 0x00, &[ 0x00, RecordRole::Filter as u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]))
			.read(&create_record(RecordType::Params, 0x01, 0x00, b"\x12\x0AFCGI_DATA_LAST_MOD1595418756\x10\x02FCGI_DATA_LENGTH12"))
			.read(&create_record(RecordType::Params, 0x01, 0x00, &[]))
			.read(&create_record(RecordType::Data, 0x01, 0x00, b"THIS_IS_DATA"))
			.read(&create_record(RecordType::Data, 0x01, 0x00, &[]))
			.read(&create_record(RecordType::StdIn, 0x01, 0x00, &[]))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 1, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(request: Arc<Request<W>>) -> RequestResult {
		assert_eq!(request.role, Role::Filter);

		// Check the parameters
		let last_mod = request.get_param("FCGI_DATA_LAST_MOD");
		assert!(last_mod.is_some());
		assert_eq!(String::from_utf8(last_mod.unwrap().to_vec()).unwrap(), "1595418756");

		let data_length = request.get_param("FCGI_DATA_LENGTH");
		assert!(data_length.is_some());
		assert_eq!(String::from_utf8(data_length.unwrap().to_vec()).unwrap(), "12");

		let mut data = [0u8; 12];
		assert!(request.get_data().read_exact(&mut data).is_ok());
		assert_eq!(String::from_utf8(Vec::from(data)).unwrap(), "THIS_IS_DATA");

		// Write some Output to verify StdOut
		RequestResult::Complete(0x00)
	}
}

pub struct TestAbortRequest {}

impl TestCase for TestAbortRequest {
	fn get_input() -> Mock {
		Builder::new()
			.read(&create_record(RecordType::BeginRequest, 0x01, 0x00, &[ 0x00, RecordRole::Responder as u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]))
			.read(&create_record(RecordType::Params, 0x01, 0x06, b"\x04\x02USERME"))
			.read(&create_record(RecordType::Params, 0x01, 0x00, &[]))
			.read(&create_record(RecordType::AbortRequest, 0x01, 0x00, &[]))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 1, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(_request: Arc<Request<W>>) -> RequestResult {
		unreachable!("This should never run because the request was aborted.");
	}
}

pub struct TestAbortContinue {}

impl TestCase for TestAbortContinue {
	fn get_input() -> Mock {
		Builder::new()
			/*Request 0*/.read(&create_record(RecordType::BeginRequest, 0x00, 0x09, &[ 0x00, 0x01, RecordFlags::KeepConn as u8, 0x00, 0x00, 0x00, 0x00, 0x00]))
			/*Request 0*/.read(&create_record(RecordType::Params, 0x00, 0x06, b"\x03\x01IDX0"))
			/*Request 1*/.read(&create_record(RecordType::BeginRequest, 0x01, 0x09, &[ 0x00, 0x01, RecordFlags::KeepConn as u8, 0x00, 0x00, 0x00, 0x00, 0x00]))
			/*Request 0*/.read(&create_record(RecordType::Params, 0x00, 0x00, &[]))
			/*Request 1*/.read(&create_record(RecordType::Params, 0x01, 0x06, b"\x03\x01IDX1"))
			/*Request 1*/.read(&create_record(RecordType::Params, 0x01, 0x00, &[]))
			/*Request 1*/.read(&create_record(RecordType::StdIn, 0x01, 0x03, &(0..100u8).collect::<Vec<u8>>()[..] ))	// Fill StdIn
			/*Request 1 Abort*/.read(&create_record(RecordType::AbortRequest, 0x01, 0x00, &[]))
			/*Request 0*/.read(&create_record(RecordType::StdIn, 0x00, 0x03, &(0..100u8).collect::<Vec<u8>>()[..] ))	// Fill StdIn
			/*Request 0*/.read(&create_record(RecordType::StdIn, 0x00, 0x03, &[]))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 1, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 0, 0, 1, 0, 0, b'0', 1, 6, 0, 0, 0, 0, 0, 0, ])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 0, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 0, 0, 8, 0, 0, 0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(request: Arc<Request<W>>) -> RequestResult {
		assert_eq!(request.get_str_param("IDX").unwrap(), "0");

		request.get_stdout().write(b"0").await.unwrap();
		RequestResult::Complete(0x44332211)
	}
}

pub struct TestUnknownRoleReturn {}

impl TestCase for TestUnknownRoleReturn {
	fn get_input() -> Mock {
		Builder::new()
			.read(&create_record(RecordType::BeginRequest, 0x01, 0x09, &[ 0x00, RecordRole::Responder as u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]))
			.read(&create_record(RecordType::Params, 0x01, 0x06, b"\x0B\x02SERVER_PORT80\x04\x03TESTYES"))
			.read(&create_record(RecordType::Params, 0x01, 0x00, &[]))
			.read(&create_record(RecordType::StdIn, 0x01, 0x03, &(0..100u8).collect::<Vec<u8>>()[..] ))	// Fill StdIn
			.read(&create_record(RecordType::StdIn, 0x01, 0x03, &[]))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 1, 0, 0, 0, 0 ])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 1, 0, 8, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(_request: Arc<Request<W>>) -> RequestResult {
		RequestResult::UnknownRole
	}
}

pub struct TestUnknownRoleRequest {}

impl TestCase for TestUnknownRoleRequest {
	fn get_input() -> Mock {
		Builder::new()
			.read(&create_record(RecordType::BeginRequest, 0x01, 0x09, &[ 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]))
			.read(&create_record(RecordType::Params, 0x01, 0x00, &[]))
			.read(&create_record(RecordType::StdIn, 0x01, 0x03, &[]))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 1, 0, 0, 0, 0 ])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 1, 0, 8, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(_request: Arc<Request<W>>) -> RequestResult {
		unreachable!("This should never run because the role is unknown.");
	}
}

pub struct TestUnknownRequestType {}

impl TestCase for TestUnknownRequestType {
	fn get_input() -> Mock {
		Builder::new()
			.read(&create_record(RecordType::InvalidType, 0x01, 0x09, &[]))
			.read(&create_record(RecordType::GetValues, 0x01, 0x00, b"\x0e\x00FCGI_MAX_CONNS"))
			.build()
	}

	fn get_output() -> Mock {
		 Builder::new()
			.write(&[ 1u8, RecordType::UnkownType as u8, 0, 1, 0, 8, 0, 0, RecordType::InvalidType as u8, 0, 0, 0, 0, 0, 0, 0 ])
			.write(&[ 1u8, RecordType::GetValuesResult as u8, 0, 1, 0, 17, 0, 0])
			.write(&[ 14u8, 1, b'F', b'C', b'G', b'I', b'_', b'M', b'A', b'X', b'_', b'C', b'O', b'N', b'N', b'S', b'5'])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(_request: Arc<Request<W>>) -> RequestResult {
		unreachable!("This should never run because the request type is unknown.");
	}
}

pub struct TestGetValues {}

impl TestCase for TestGetValues {
	fn get_input() -> Mock {
		Builder::new()
			.read(&create_record(RecordType::GetValues, 0x01, 0x00, b"\x0e\x00FCGI_MAX_CONNS\x0d\x00FCGI_MAX_REQS\x0f\x00FCGI_MPXS_CONNS"))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			.write(&[ 1u8, RecordType::GetValuesResult as u8, 0, 1, 0, 52, 0, 0])
			.write(&[ 14u8, 1, b'F', b'C', b'G', b'I', b'_', b'M', b'A', b'X', b'_', b'C', b'O', b'N', b'N', b'S', b'5'])
			.write(&[ 13u8, 2, b'F', b'C', b'G', b'I', b'_', b'M', b'A', b'X', b'_', b'R', b'E', b'Q', b'S', b'1', b'0'])
			.write(&[ 15u8, 1, b'F', b'C', b'G', b'I', b'_', b'M', b'P', b'X', b'S', b'_', b'C', b'O', b'N', b'N', b'S', b'1'])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(_request: Arc<Request<W>>) -> RequestResult {
		unreachable!("This should never run because a get_value request should never run the processor.");
	}
}

pub struct TestKeepConnection {
}

impl TestCase for TestKeepConnection {
	fn get_input() -> Mock {
		Builder::new()
			/*Request 0*/.read(&create_record(RecordType::BeginRequest, 0x00, 0x09, &[ 0x00, 0x01, RecordFlags::KeepConn as u8, 0x00, 0x00, 0x00, 0x00, 0x00]))
			/*Request 0*/.read(&create_record(RecordType::Params, 0x00, 0x06, b"\x03\x01IDX1"))
			/*Request 1*/.read(&create_record(RecordType::BeginRequest, 0x01, 0x09, &[ 0x00, 0x01, RecordFlags::KeepConn as u8, 0x00, 0x00, 0x00, 0x00, 0x00]))
			/*Request 0*/.read(&create_record(RecordType::Params, 0x00, 0x00, &[]))
			/*Random Delay*/.wait(Duration::from_millis(100))
			/*Request 1*/.read(&create_record(RecordType::Params, 0x01, 0x06, b"\x03\x01IDX2"))
			/*Request 1*/.read(&create_record(RecordType::Params, 0x01, 0x00, &[]))
			/*Request 1*/.read(&create_record(RecordType::StdIn, 0x01, 0x03, &(0..100u8).collect::<Vec<u8>>()[..] ))	// Fill StdIn
			/*Random Delay*/.wait(Duration::from_millis(100))
			/*Request 1*/.read(&create_record(RecordType::StdIn, 0x01, 0x03, &[]))
			/*Request 0*/.read(&create_record(RecordType::StdIn, 0x00, 0x03, &(0..100u8).collect::<Vec<u8>>()[..] ))	// Fill StdIn
			/*Request 0*/.read(&create_record(RecordType::StdIn, 0x00, 0x03, &[]))
			.build()
	}

	fn get_output() -> Mock {
		Builder::new()
			/* Request 1 */
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 1, 0, 1, 0, 0, b'2'])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 1, 0, 2, 0, 0, b'X', b'B'])
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 1, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 1, 0, 8, 0, 0, 0x22, 0x44, 0x66, 0x88, 0, 0, 0, 0])
			/* Request 0 */
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 0, 0, 1, 0, 0, b'1'])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 0, 0, 2, 0, 0, b'X', b'A'])
			.write(&[ 1u8, RecordType::StdOut as u8, 0, 0, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::StdErr as u8, 0, 0, 0, 0, 0, 0])
			.write(&[ 1u8, RecordType::EndRequest as u8, 0, 0, 0, 8, 0, 0, 0x11, 0x22, 0x33, 0x44, 0, 0, 0, 0])
			.build()
	}

	async fn processor<W: AsyncWrite + Unpin + Send>(request: Arc<Request<W>>) -> RequestResult {
		let idx = request.get_str_param("IDX").unwrap();

		request.get_stdout().write(idx.as_bytes()).await.unwrap();
		request.get_stderr().write(&[b'X', idx.as_bytes()[0] - b'1' + b'A']).await.unwrap();
		RequestResult::Complete(0x11223344 * idx.parse().unwrap_or(0))
	}
}
