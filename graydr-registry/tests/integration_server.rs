use std::sync::Arc;
use graydr_registry::{AppState, config::ServerConfig, store::FilesystemStore, routes::build_router};

async fn start_test_server() -> (u16, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage_dir = tmp.path().to_path_buf();
    let config = Arc::new(ServerConfig::new(0, storage_dir.clone()));
    let store = Arc::new(FilesystemStore::new(storage_dir));
    let state = Arc::new(AppState { store, config });
    let router = build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });
    (port, tmp)
}

#[tokio::test]
async fn test_serve_starts_and_logs() {
    // SRV-01: server binds port 0, GET / returns 404 (not a connection error), confirming server is up and reachable
    let (port, _tmp) = start_test_server().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/", port))
        .send()
        .await
        .unwrap();
    // 404 is expected (no route at /); the point is the server is up
    assert_eq!(resp.status().as_u16(), 404, "server must be reachable");
}

#[tokio::test]
async fn test_publish_stores_module() {
    // SRV-02: PUT module → 200; file exists on disk at correct path
    let (port, tmp) = start_test_server().await;

    let module_content = b"module \"networking\" { metadata { version = \"1.0.0\" } }";
    let form = reqwest::multipart::Form::new().part(
        "module",
        reqwest::multipart::Part::bytes(module_content.as_ref())
            .file_name("networking.gmod")
            .mime_str("application/octet-stream")
            .unwrap(),
    );

    let client = reqwest::Client::new();
    let resp = client
        .put(format!(
            "http://127.0.0.1:{}/api/v1/modules/testorg/networking/1.0.0",
            port
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        200,
        "publish must return 200; body: {}",
        resp.text().await.unwrap_or_default()
    );

    // Verify files exist on disk
    let module_path = tmp
        .path()
        .join("testorg")
        .join("networking")
        .join("1.0.0")
        .join("module.gmod");
    let meta_path = tmp
        .path()
        .join("testorg")
        .join("networking")
        .join("1.0.0")
        .join("meta.json");
    assert!(
        module_path.exists(),
        "module.gmod must exist at {}",
        module_path.display()
    );
    assert!(
        meta_path.exists(),
        "meta.json must exist at {}",
        meta_path.display()
    );

    // Verify meta lifecycle is "active" (lowercase)
    let meta_raw = std::fs::read_to_string(&meta_path).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&meta_raw).unwrap();
    assert_eq!(
        meta["lifecycle"].as_str(),
        Some("active"),
        "lifecycle must be lowercase 'active'; got: {}",
        meta_raw
    );
}

#[tokio::test]
async fn test_publish_duplicate_returns_409() {
    // SRV-04: second PUT to same coordinate must return 409 Conflict
    let (port, _tmp) = start_test_server().await;

    let content = b"module content";
    let client = reqwest::Client::new();

    let put = |port: u16| {
        let client = client.clone();
        async move {
            let form = reqwest::multipart::Form::new().part(
                "module",
                reqwest::multipart::Part::bytes(content.as_ref())
                    .file_name("mod.gmod")
                    .mime_str("application/octet-stream")
                    .unwrap(),
            );
            client
                .put(format!(
                    "http://127.0.0.1:{}/api/v1/modules/duporg/dupmod/1.0.0",
                    port
                ))
                .multipart(form)
                .send()
                .await
                .unwrap()
        }
    };

    let first = put(port).await;
    assert_eq!(first.status().as_u16(), 200, "first publish must succeed");

    let second = put(port).await;
    assert_eq!(
        second.status().as_u16(),
        409,
        "duplicate publish must return 409 Conflict"
    );
}

#[tokio::test]
async fn test_content_returns_stored_bytes() {
    // SRV-03: GET content returns exactly what was published
    let (port, _tmp) = start_test_server().await;

    let original = b"module \"mymod\" { }";
    let form = reqwest::multipart::Form::new().part(
        "module",
        reqwest::multipart::Part::bytes(original.as_ref())
            .file_name("mymod.gmod")
            .mime_str("application/octet-stream")
            .unwrap(),
    );

    let client = reqwest::Client::new();
    client
        .put(format!(
            "http://127.0.0.1:{}/api/v1/modules/contentorg/mymod/2.0.0",
            port
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/modules/contentorg/mymod/2.0.0/content",
            port
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.as_ref(), original, "content must match published bytes");
}

#[tokio::test]
async fn test_meta_returns_active_lifecycle() {
    // SRV-03: GET meta returns JSON with lifecycle: "active" after publish
    let (port, _tmp) = start_test_server().await;

    let form = reqwest::multipart::Form::new().part(
        "module",
        reqwest::multipart::Part::bytes(b"module content".as_ref())
            .file_name("mod.gmod")
            .mime_str("application/octet-stream")
            .unwrap(),
    );

    let client = reqwest::Client::new();
    client
        .put(format!(
            "http://127.0.0.1:{}/api/v1/modules/metaorg/metamod/3.0.0",
            port
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/modules/metaorg/metamod/3.0.0/meta",
            port
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["lifecycle"].as_str(),
        Some("active"),
        "lifecycle must be lowercase 'active'; got: {:?}",
        body
    );
    assert_eq!(body["org"].as_str(), Some("metaorg"));
    assert_eq!(body["name"].as_str(), Some("metamod"));
    assert_eq!(body["version"].as_str(), Some("3.0.0"));
}
