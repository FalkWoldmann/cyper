mod server;

#[cfg(feature = "json")]
use std::collections::HashMap;

use cyper::Client;
#[cfg(feature = "json")]
use http::header::CONTENT_TYPE;

#[compio::test]
async fn response_text() {
    let server = server::http(move |_req| async { "Hello" }).await;

    let client = Client::new().unwrap();

    let res = client
        .get(format!("http://{}/text", server.addr()))
        .expect("cannot create request builder")
        .send()
        .await
        .expect("Failed to get");
    assert_eq!(res.content_length(), Some(5));
    let text = res.text().await.expect("Failed to get text");
    assert_eq!("Hello", text);
}

#[compio::test]
async fn response_bytes() {
    let server = server::http(move |_req| async { "Hello" }).await;

    let client = Client::new().unwrap();

    let res = client
        .get(format!("http://{}/bytes", server.addr()))
        .expect("cannot create request builder")
        .send()
        .await
        .expect("Failed to get");
    assert_eq!(res.content_length(), Some(5));
    let bytes = res.bytes().await.expect("res.bytes()");
    assert_eq!("Hello", bytes);
}

#[compio::test]
#[cfg(feature = "stream")]
async fn response_bytes_stream() {
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    use axum::body::Body;
    use compio::bytes;
    use futures_util::{Stream, StreamExt};

    #[derive(Default)]
    struct ChunkedBody {
        chunks_sent: usize,
    }

    impl Stream for ChunkedBody {
        type Item = Result<bytes::Bytes, std::io::Error>;

        fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            if self.chunks_sent == 10 {
                return Poll::Ready(None);
            }

            let this = self.get_mut();

            this.chunks_sent += 1;
            Poll::Ready(Some(Ok(format!("Chunk {}", this.chunks_sent).into())))
        }
    }

    let server =
        server::http(move |_req| async { Body::from_stream(ChunkedBody::default()) }).await;

    let client = Client::new().unwrap();

    let res = client
        .get(format!("http://{}/bytes", server.addr()))
        .expect("cannot create request builder")
        .send()
        .await
        .expect("Failed to get");
    assert_eq!(res.content_length(), None);

    let mut bytes_stream = res.bytes_stream().enumerate();
    let mut chunks_received = 0;

    while let Some((i, bytes)) = bytes_stream.next().await {
        chunks_received += 1;
        assert_eq!(
            bytes.unwrap(),
            compio::bytes::Bytes::from(format!("Chunk {}", i + 1))
        );
    }

    assert_eq!(chunks_received, 10);
}

#[compio::test]
#[cfg(feature = "json")]
async fn response_json() {
    let server = server::http(move |_req| async { "\"Hello\"" }).await;

    let client = Client::new().unwrap();

    let res = client
        .get(format!("http://{}/json", server.addr()))
        .expect("cannot create request builder")
        .send()
        .await
        .expect("Failed to get");
    let text = res.json::<String>().await.expect("Failed to get json");
    assert_eq!("Hello", text);
}

#[compio::test]
#[cfg(feature = "stream")]
async fn service() {
    use http::Method;
    use http_body_util::BodyExt;
    use tower_service::Service;

    let server = server::http(move |_req| async { "Hello" }).await;

    let mut client = Client::new().unwrap();

    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("http://{}/text", server.addr()))
        .body(cyper::Body::empty())
        .unwrap();

    let res = client.call(request).await.expect("Failed to get");
    let content_length = res
        .headers()
        .get(http::header::CONTENT_LENGTH)
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(content_length, 5);
    let text = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!("Hello", text);
}

#[compio::test]
#[cfg(tls)]
async fn test_allowed_methods() {
    let resp = Client::new()
        .unwrap()
        .get("https://compio.rs")
        .unwrap()
        .send()
        .await;

    assert!(resp.is_ok());
}

#[compio::test]
#[cfg(feature = "native-tls")]
async fn test_native_tls() {
    let resp = Client::builder()
        .use_native_tls()
        .build()
        .unwrap()
        .get("https://compio.rs")
        .unwrap()
        .send()
        .await
        .unwrap();
    resp.text().await.unwrap();
}

#[compio::test]
#[cfg(feature = "rustls")]
async fn test_rustls() {
    let resp = Client::builder()
        .use_rustls_default()
        .build()
        .unwrap()
        .get("https://compio.rs")
        .unwrap()
        .send()
        .await
        .unwrap();
    resp.text().await.unwrap();
}

#[compio::test]
#[cfg(feature = "http3")]
async fn test_http3() {
    let resp = Client::builder()
        .build()
        .unwrap()
        .get("https://compio.rs")
        .unwrap()
        .version(http::Version::HTTP_3)
        .send()
        .await
        .unwrap();
    resp.text().await.unwrap();
}

#[compio::test]
#[cfg(feature = "json")]
async fn add_json_default_content_type_if_not_set_manually() {
    let mut map = HashMap::new();
    map.insert("body", "json");
    let content_type = http::HeaderValue::from_static("application/vnd.api+json");
    let req = Client::new()
        .unwrap()
        .post("https://compio.rs/")
        .expect("cannot create request builder")
        .header(CONTENT_TYPE, &content_type)
        .unwrap()
        .json(&map)
        .unwrap()
        .build();

    assert_eq!(content_type, req.headers().get(CONTENT_TYPE).unwrap());
}

#[compio::test]
#[cfg(feature = "json")]
async fn update_json_content_type_if_set_manually() {
    let mut map = HashMap::new();
    map.insert("body", "json");
    let req = Client::new()
        .unwrap()
        .post("https://compio.rs/")
        .expect("cannot create request builder")
        .json(&map)
        .unwrap()
        .build();

    assert_eq!("application/json", req.headers().get(CONTENT_TYPE).unwrap());
}

#[compio::test]
pub async fn accept_encoding_header_is_not_changed_if_set() {
    let server = server::http(move |req: http::Request<axum::body::Body>| async move {
        assert_eq!(req.headers()["accept"], "*/*");
        assert_eq!(req.headers()["accept-encoding"], "identity");
        http::Response::<axum::body::Body>::default()
    })
    .await;

    let client = Client::new().unwrap();

    let res = client
        .get(format!("http://{}/accept-encoding", server.addr()))
        .unwrap()
        .header("accept-encoding", "identity")
        .unwrap()
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), http::StatusCode::OK);
}

#[compio::test]
#[cfg(tls)]
async fn connect_timeout_on_stalled_handshake() {
    use std::time::Duration;

    // The kernel completes the TCP handshake, but nobody answers the TLS one.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let client = Client::builder()
        .connect_timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();
    let res = compio::time::timeout(
        Duration::from_secs(5),
        client.get(format!("https://{addr}")).unwrap().send(),
    )
    .await
    .expect("connect timeout didn't fire");

    assert!(matches!(res, Err(cyper::Error::Timeout)), "{res:?}");
}

#[compio::test]
async fn connect_timeout_ignores_slow_response() {
    use std::time::Duration;

    let server = server::http(move |_req| async {
        // axum wants a `Send` handler.
        send_wrapper::SendWrapper::new(compio::time::sleep(Duration::from_millis(300))).await;
        "Hello"
    })
    .await;

    let client = Client::builder()
        .connect_timeout(Duration::from_millis(100))
        .no_proxy()
        .build()
        .unwrap();
    let text = client
        .get(format!("http://{}/slow", server.addr()))
        .unwrap()
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(text, "Hello");
}
