// This crate is a library
#![crate_type = "lib"]
// The library is named "maestro-local-api"
#![crate_name = "maestro_local_api"]

pub mod maestro;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

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
        let socket_path = "/run/maestro.sock";
        if let Ok(mut stream) = UnixStream::connect(socket_path) {
            stream
                .set_read_timeout(Some(Duration::from_secs(15)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(15)))
                .unwrap();
            stream.set_nonblocking(false).unwrap();

            if let Err(e) = stream.write_all(command.as_bytes()) {
                return Response::ERROR(format!("Failed to write command: {:?}", e));
            }

            // can close the write end now
            stream
                .shutdown(std::net::Shutdown::Write)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to shutdown client stream on write: {:?}", e);
                });

            let mut response = String::new();
            let mut reader = BufReader::new(&mut stream);
            if reader.read_line(&mut response).is_ok() {
                stream
                    .shutdown(std::net::Shutdown::Read)
                    .unwrap_or_else(|e| {
                        eprintln!("Failed to shutdown client stream on read: {:?}", e);
                    });

                if let Some(stripped) = response.strip_prefix("OK ") {
                    let rest = stripped.trim().trim_end_matches(&['\n', '\r'][..]).trim();
                    if rest == "YES" || rest.is_empty() {
                        Response::OK(true)
                    } else if rest == "NO" {
                        Response::OK(false)
                    } else {
                        Response::OkResponse(rest.to_string())
                    }
                } else if response == "OK" || response == "OK\n" || response == "OK\r\n" {
                    Response::OK(true)
                } else if let Some(stripped) = response.strip_prefix("ERROR ") {
                    let rest = stripped.trim().trim_end_matches(&['\n', '\r'][..]).trim();
                    Response::ERROR(rest.to_string())
                } else {
                    Response::ERROR(
                        response
                            .trim()
                            .trim_end_matches(&['\n', '\r'][..])
                            .trim()
                            .to_string(),
                    )
                }
            } else {
                Response::ERROR("Failed to read response".to_string())
            }
        } else {
            Response::ERROR(format!("Failed to connect to socket at {}", socket_path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
