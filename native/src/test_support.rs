//! Small local HTTP fixture. Never reaches official services or physical cards.
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
pub fn api(responses: Vec<Value>) -> (crate::api::Api, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let mut requests = vec![];
        for body in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut raw = vec![];
            let mut byte = [0];
            while !raw.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                raw.push(byte[0]);
            }
            let header = String::from_utf8(raw).unwrap();
            let length = header
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut data = vec![0; length];
            stream.read_exact(&mut data).unwrap();
            requests.push(format!(
                "{}\n{}",
                header.lines().next().unwrap(),
                String::from_utf8_lossy(&data)
            ));
            let body = body.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        }
        requests
    });
    (crate::api::Api::mock(base), worker)
}
