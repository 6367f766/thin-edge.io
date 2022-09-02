use futures::StreamExt;
use hyper::{server::conn::AddrIncoming, Request};
use hyper::{Body, Response, Server};
use routerify::{Router, RouterService};
use std::{io::Write, path::Path};
use std::{net::SocketAddr, path::PathBuf};

use tedge_utils::paths::create_directories;
use tokio::io::AsyncReadExt;

use crate::error::AgentError;
const FILE_TRANSFER_HTTP_ENDPOINT: &str = "/tedge/*";

fn separate_path_and_file_name(input: &str) -> Option<(PathBuf, &str)> {
    let (mut relative_path, file_name) = input.rsplit_once('/')?;

    if relative_path.starts_with('/') {
        relative_path = &relative_path[1..];
    }
    let relative_path = PathBuf::from(relative_path);
    Some((relative_path, file_name))
}

async fn put(mut request: Request<Body>, root_path: &str) -> Result<Response<Body>, AgentError> {
    let uri = request.uri().to_string();
    let mut response = Response::new(Body::empty());
    dbg!(&uri);
    if let Some((relative_path, file_name)) = separate_path_and_file_name(&uri) {
        let root_path = PathBuf::from(root_path);
        let directories_path = root_path.join(relative_path);

        if let Err(_err) = create_directories(&directories_path) {
            *response.status_mut() = hyper::StatusCode::FORBIDDEN;
        }

        let full_path = directories_path.join(file_name);

        match stream_request_body_to_path(&full_path, request.body_mut()).await {
            Ok(()) => {
                *response.status_mut() = hyper::StatusCode::CREATED;
            }
            Err(_err) => {
                *response.status_mut() = hyper::StatusCode::FORBIDDEN;
            }
        }
    } else {
        *response.status_mut() = hyper::StatusCode::FORBIDDEN;
    }
    Ok(response)
}

async fn get(request: Request<Body>, root_path: &str) -> Result<Response<Body>, AgentError> {
    let full_path = PathBuf::from(format!("{}{}", root_path, request.uri()));
    dbg!("get path", &full_path);
    if !full_path.exists() || full_path.is_dir() {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = hyper::StatusCode::NOT_FOUND;
        return Ok(response);
    }

    let mut file = tokio::fs::File::open(full_path).await?;

    let mut contents = vec![];
    file.read_to_end(&mut contents).await?;

    let output = String::from_utf8(contents)?;

    Ok(Response::new(Body::from(output)))
}

async fn delete(request: Request<Body>, root_path: &str) -> Result<Response<Body>, AgentError> {
    let full_path = PathBuf::from(format!("{}{}", root_path, request.uri()));

    let mut response = Response::new(Body::empty());

    dbg!("delete full path", &full_path);

    if !full_path.exists() {
        dbg!("full_path did not exist");
        *response.status_mut() = hyper::StatusCode::INTERNAL_SERVER_ERROR;
        Ok(response)
    } else {
        match tokio::fs::remove_file(&full_path).await {
            Ok(()) => {
                *response.status_mut() = hyper::StatusCode::ACCEPTED;
                Ok(response)
            }
            Err(_err) => {
                *response.status_mut() = hyper::StatusCode::INTERNAL_SERVER_ERROR;
                Ok(response)
            }
        }
    }
}

async fn stream_request_body_to_path(
    path: &Path,
    body_stream: &mut Body,
) -> Result<(), AgentError> {
    let mut buffer = std::fs::File::create(path)?;
    while let Some(data) = body_stream.next().await {
        let data = data?;
        let _bytes_written = buffer.write(&data)?;
    }
    Ok(())
}

pub fn http_file_transfer_server(
    bind_address: &str,
    root_path: &'static str,
) -> Server<AddrIncoming, RouterService<Body, AgentError>> {
    // TODO: error handling
    // Not sure what the best way to handle unwraps here is.
    if let Ok(router) = Router::builder()
        .get(FILE_TRANSFER_HTTP_ENDPOINT, |req| get(req, root_path))
        .put(FILE_TRANSFER_HTTP_ENDPOINT, |req| put(req, root_path))
        .delete(FILE_TRANSFER_HTTP_ENDPOINT, |req| delete(req, root_path))
        .build()
    {
        let router_service = RouterService::new(router).unwrap();

        let mut addr: SocketAddr = bind_address.parse().unwrap();
        addr.set_port(3000);

        Server::bind(&addr).serve(router_service)
    } else {
        todo!()
    }
}

#[cfg(test)]
mod test {

    use super::{http_file_transfer_server, separate_path_and_file_name};
    use crate::error::AgentError;
    use hyper::{server::conn::AddrIncoming, Body, Method, Request, Server};
    use lazy_static::lazy_static;
    use routerify::RouterService;
    use tedge_test_utils::fs::TempTedgeDir;

    #[rstest::rstest]
    #[case("/tedge", "", "tedge")]
    #[case("/tedge/some/dir/file", "tedge/some/dir", "file")]
    #[case("/tedge/some/dir/", "tedge/some/dir", "")]
    fn test_separate_path_and_file_name(
        #[case] input: &str,
        #[case] expected_path: &str,
        #[case] expected_file_name: &str,
    ) {
        let (actual_path, actual_file_name) = separate_path_and_file_name(input).unwrap();
        assert_eq!(actual_path, std::path::PathBuf::from(expected_path));
        assert_eq!(actual_file_name, expected_file_name);
    }

    const VALID_TEST_URI: &'static str = "http://127.0.0.1:3000/tedge/another/dir/test-file";
    const INVALID_TEST_URI: &'static str = "http://127.0.0.1:3000/wrong/place/test-file";

    #[rstest::rstest]
    #[case(VALID_TEST_URI, hyper::StatusCode::OK)]
    #[case(INVALID_TEST_URI, hyper::StatusCode::NOT_FOUND)]
    #[tokio::test]
    #[serial_test::serial]
    async fn test_file_transfer_get(
        #[case] uri: &'static str,
        #[case] status_code: hyper::StatusCode,
    ) {
        let client_handler = tokio::spawn(async move {
            let client = hyper::Client::new();
            let response = client.get(hyper::Uri::from_static(uri)).await.unwrap();
            response
        });

        let (_ttd, server) = server_fixture().await;
        tokio::select! {
            _out = server => {
            }
            Ok(response) = client_handler => {
                assert_eq!(response.status(), status_code);
            }
        };
    }

    #[rstest::rstest]
    #[case(VALID_TEST_URI, hyper::StatusCode::ACCEPTED)]
    #[case(INVALID_TEST_URI, hyper::StatusCode::NOT_FOUND)]
    #[tokio::test]
    #[serial_test::serial]
    async fn test_file_transfer_delete(
        #[case] uri: &'static str,
        #[case] status_code: hyper::StatusCode,
    ) {
        let client_handler = tokio::spawn(async move {
            let client = hyper::Client::new();

            let req = Request::builder()
                .method(Method::DELETE)
                .uri(uri)
                .body(Body::empty())
                .expect("request builder");
            client.request(req).await.unwrap()
        });

        let (_ttd, server) = server_fixture().await;
        tokio::select! {
            _out = server => {
            }
            Ok(response) = client_handler => {
                //dbg!(std::path::PathBuf::from(&TMPPATH.clone()).join("tedge/another/dir/test-file").exists());
                assert_eq!(response.status(), status_code);
            }

        };
    }

    #[rstest::rstest]
    #[case(VALID_TEST_URI, hyper::StatusCode::CREATED)]
    #[case(INVALID_TEST_URI, hyper::StatusCode::NOT_FOUND)] // TODO what about 403
    #[tokio::test]
    #[serial_test::serial]
    async fn test_file_transfer_put(
        #[case] uri: &'static str,
        #[case] status_code: hyper::StatusCode,
    ) {
        let put_request_handler = tokio::spawn(async move {
            let client = hyper::Client::new();

            let mut string = String::new();
            for val in 0..100 {
                string.push_str(&format!("{}\n", val));
            }
            let req = Request::builder()
                .method(Method::PUT)
                .uri(uri)
                .body(Body::from(string.clone()))
                .expect("request builder");
            let response = client.request(req).await.unwrap();
            response
        });

        let (_ttd, server) = server_fixture().await;

        tokio::select! {
            _out = server => {
            }
            Ok(_put_response) = put_request_handler => {
                assert_eq!(_put_response.status(), status_code);
            }
        }
    }

    #[rstest::fixture]
    async fn get_request_fixture() -> hyper::Response<Body> {
        let client = hyper::Client::new();
        let request = client
            .get(hyper::Uri::from_static(VALID_TEST_URI))
            .await
            .unwrap();
        request
    }

    async fn server_fixture() -> (
        TempTedgeDir,
        Server<AddrIncoming, RouterService<Body, AgentError>>,
    ) {
        lazy_static! {
            static ref TTD: TempTedgeDir = {
                let ttd = TempTedgeDir::new();
                let file = ttd.dir("tedge").dir("another").dir("dir").file("test-file");
                file.with_raw_content("raw content");
                ttd
            };
            static ref TMPPATH: String = {
                let tempdir_path = String::from(TTD.path().to_str().unwrap());
                tempdir_path
            };
        }

        let ttd = TempTedgeDir::new();
        let file = ttd.dir("tedge").dir("another").dir("dir").file("test-file");
        file.with_raw_content("raw content");
        let tempdir_path = String::from(ttd.path().to_str().unwrap());
        let leaked: &'static str = Box::leak(tempdir_path.into_boxed_str());
        (ttd, http_file_transfer_server("127.0.0.1:3000", &leaked))
    }

    #[rstest::fixture]
    async fn put_request_fixture() -> hyper::client::ResponseFuture {
        let client = hyper::Client::new();

        let mut string = String::new();
        for val in 0..100 {
            string.push_str(&format!("{}\n", val));
        }
        let req = Request::builder()
            .method(Method::PUT)
            .uri(VALID_TEST_URI)
            .body(Body::from(string.clone()))
            .expect("request builder");
        //let out = client.request(req).await.unwrap();
        client.request(req)
    }
}
