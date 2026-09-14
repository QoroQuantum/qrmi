// This crate is a library
#![crate_type = "lib"]
// The library is named "maestro-local-api"
#![crate_name = "maestro_local_api"]

pub mod maestro;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use std::time::Duration;

pub mod maestro_lib {
    use super::*;

    #[derive(Debug)]
    pub enum Response {
        OK(bool),
        ERROR(String),
        OkResponse(String),
    }

    pub async fn ping() -> Response {
        send_command_close("PING\n")
    }

    pub fn send_command_close(command: &str) -> Response {
        let socket_path =
            std::env::var_os("QRMI_MAESTRO_SOCKET").unwrap_or_else(|| "/run/maestro.sock".into());
        match exchange(Path::new(&socket_path), command) {
            Ok(response) => parse_response(&response),
            Err(error) => Response::ERROR(format!(
                "Maestro socket {}: {error}",
                Path::new(&socket_path).display()
            )),
        }
    }

    fn exchange(socket_path: &Path, command: &str) -> std::io::Result<String> {
        let mut stream = UnixStream::connect(socket_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(15)))?;
        stream.set_write_timeout(Some(Duration::from_secs(15)))?;
        stream.write_all(command.as_bytes())?;
        stream.shutdown(std::net::Shutdown::Write)?;
        let mut response = String::new();
        if BufReader::new(stream).read_line(&mut response)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "server closed the connection without a response",
            ));
        }
        Ok(response)
    }

    fn parse_response(response: &str) -> Response {
        let response = response.trim();
        if response == "OK" {
            return Response::OK(true);
        }
        if let Some(rest) = response.strip_prefix("OK ") {
            match rest.trim() {
                "YES" | "" => Response::OK(true),
                "NO" => Response::OK(false),
                value => Response::OkResponse(value.to_string()),
            }
        } else if let Some(rest) = response.strip_prefix("ERROR ") {
            Response::ERROR(rest.trim().to_string())
        } else {
            Response::ERROR(format!("Unexpected response: {response:?}"))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_acknowledgements_values_and_errors() {
            assert!(matches!(parse_response("OK YES\r\n"), Response::OK(true)));
            assert!(matches!(parse_response("OK\n"), Response::OK(true)));
            assert!(matches!(parse_response("OK NO\n"), Response::OK(false)));
            assert!(matches!(parse_response("OK 123\n"), Response::OkResponse(id) if id == "123"));
            assert!(
                matches!(parse_response("ERROR refused\n"), Response::ERROR(message) if message == "refused")
            );
            assert!(
                matches!(parse_response("broken\n"), Response::ERROR(message) if message.contains("Unexpected response"))
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a running Maestro server at /run/maestro.sock"]
    fn simple_test() {
        let response = maestro_lib::send_command_close("SESSION 0 EXISTS");
        assert!(
            matches!(response, maestro_lib::Response::OK(false)),
            "Expected OK NO response, got: {:?}",
            response
        );

        let ping_response = futures::executor::block_on(maestro_lib::ping());
        assert!(
            matches!(ping_response, maestro_lib::Response::OK(true)),
            "Expected OK YES response, got: {:?}",
            ping_response
        );
    }
}
