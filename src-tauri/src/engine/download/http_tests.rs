use super::*;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

async fn server(responses: Vec<&'static str>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/file", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (socket, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept()).await.unwrap().unwrap();
            let mut socket = BufReader::new(socket);
            let mut request = String::new();
            loop {
                let mut line = String::new();
                let n = tokio::time::timeout(Duration::from_secs(5), socket.read_line(&mut line)).await.unwrap().unwrap();
                request.push_str(&line);
                if n == 0 || line == "\r\n" {
                    break;
                }
            }
            requests.push(request.to_ascii_lowercase());
            socket.get_mut().write_all(response.as_bytes()).await.unwrap();
            socket.get_mut().shutdown().await.unwrap();
        }
        requests
    });
    (url, handle)
}

fn task(url: String, path: PathBuf) -> DownloadTask {
    DownloadTask { url, path, sha1: None, size: Some(5) }
}

fn downloader() -> Downloader {
    let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap();
    let mut downloader = Downloader::new(client, Source::Official, 1);
    downloader.retries = 1;
    downloader
}

#[test]
fn validates_complete_suffix_content_ranges() {
    assert_eq!(resume_size("bytes 2-4/5", 2, Some(5)), Some(5));
    assert_eq!(resume_size("bytes 2-4/5", 2, None), Some(5));
    assert_eq!(resume_size("bytes 2-4/*", 2, Some(5)), Some(5));
    for value in [
        "bytes 0-4/5", "bytes 2-3/5", "bytes 2-5/6", "bytes 4-2/5",
        "bytes 2-4/*", "items 2-4/5", "bytes +2-4/5", "bytes 2-18446744073709551615/0",
    ] {
        let expected = if value == "bytes 2-4/*" { None } else { Some(5) };
        assert_eq!(resume_size(value, 2, expected), None, "{value}");
    }
}

#[tokio::test]
async fn resumes_at_the_requested_offset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(part_path(&path), b"he").await.unwrap();
    let (url, server) = server(vec!["HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 2-4/5\r\nContent-Length: 3\r\nConnection: close\r\n\r\nllo"]).await;
    let counters = Counters::default();
    let mut counted = 0;
    downloader().fetch(&url, &task(url.clone(), path.clone()), &counters, &mut counted).await.unwrap();
    assert_eq!(fs::read(&path).await.unwrap(), b"hello");
    assert_eq!(counted, 5);
    assert!(server.await.unwrap()[0].contains("range: bytes=2-"));
    assert!(!part_path(&path).exists());
}

#[tokio::test]
async fn restarts_when_server_ignores_range() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(part_path(&path), b"xx").await.unwrap();
    let (url, server) = server(vec!["HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello"]).await;
    let mut counted = 0;
    downloader().fetch(&url, &task(url.clone(), path.clone()), &Counters::default(), &mut counted).await.unwrap();
    assert_eq!(fs::read(&path).await.unwrap(), b"hello");
    assert_eq!(counted, 5);
    assert!(server.await.unwrap()[0].contains("range: bytes=2-"));
}

#[tokio::test]
async fn invalid_range_clears_partial_and_retries_from_scratch() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(part_path(&path), b"he").await.unwrap();
    let (url, server) = server(vec![
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-2/5\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad",
        "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
    ]).await;
    let mut downloader = downloader();
    downloader.retries = 2;
    let report = downloader.download_all(vec![task(url, path.clone())], Arc::new(|_| {})).await;
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert_eq!(report.downloaded, 1);
    assert_eq!(fs::read(&path).await.unwrap(), b"hello");
    let requests = server.await.unwrap();
    assert!(requests[0].contains("range: bytes=2-"));
    assert!(!requests[1].contains("range:"));
}

#[tokio::test]
async fn rejects_partial_response_without_a_content_range() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(part_path(&path), b"he").await.unwrap();
    let (url, server) = server(vec!["HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nConnection: close\r\n\r\nllo"]).await;
    let result = downloader().fetch(&url, &task(url.clone(), path.clone()), &Counters::default(), &mut 0).await;
    assert!(result.is_err());
    assert!(!path.exists());
    assert!(!part_path(&path).exists());
    server.await.unwrap();
}

#[tokio::test]
async fn checks_size_even_when_sha1_matches() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    let (url, server) = server(vec!["HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello"]).await;
    let mut task = task(url.clone(), path.clone());
    task.sha1 = Some("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".into());
    task.size = Some(6);
    assert!(downloader().fetch(&url, &task, &Counters::default(), &mut 0).await.is_err());
    assert!(!path.exists());
    assert!(!part_path(&path).exists());
    server.await.unwrap();
}

#[tokio::test]
async fn unknown_task_size_still_requires_a_complete_range_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(part_path(&path), b"he").await.unwrap();
    let (url, server) = server(vec!["HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 2-4/5\r\nContent-Length: 2\r\nConnection: close\r\n\r\nll"]).await;
    let mut task = task(url.clone(), path.clone());
    task.size = None;
    assert!(downloader().fetch(&url, &task, &Counters::default(), &mut 0).await.is_err());
    assert!(!path.exists());
    server.await.unwrap();
}

#[tokio::test]
async fn directories_are_not_complete_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let mut task = task(String::new(), dir.path().to_path_buf());
    task.size = None;
    assert!(!is_complete(&task, false).await);
}

#[test]
fn download_keys_do_not_follow_other_hosts_or_downgrades() {
    let official = reqwest::Url::parse("https://edge.forgecdn.net/files/1/2/mod.jar").unwrap();
    let request = download_request(&official, 12, Some("local-test-key")).unwrap();
    assert_eq!(request.headers()["x-api-key"], "local-test-key");
    assert!(request.headers()["x-api-key"].is_sensitive());
    assert_eq!(request.headers()[header::RANGE], "bytes=12-");

    for next in [
        "https://mod.mcimirror.top/files/1/2/mod.jar",
        "https://cdn.modrinth.com/mod.jar",
        "https://redirect.example.test/mod.jar",
        "http://edge.forgecdn.net/files/1/2/mod.jar",
    ] {
        let request = download_request(&reqwest::Url::parse(next).unwrap(), 12, Some("local-test-key")).unwrap();
        assert!(!request.headers().contains_key("x-api-key"), "{next}");
        assert_eq!(request.headers()[header::RANGE], "bytes=12-");
    }
}

#[tokio::test]
async fn configured_keys_are_not_sent_to_an_unrelated_download_server() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    let (url, server) = server(vec!["HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello"]).await;
    let downloader = downloader().with_curseforge_key(Some("local-test-key"));
    downloader.fetch(&url, &task(url.clone(), path), &Counters::default(), &mut 0).await.unwrap();
    let request = &server.await.unwrap()[0];
    assert!(!request.contains("x-api-key"));
    assert!(!request.contains("local-test-key"));
}
