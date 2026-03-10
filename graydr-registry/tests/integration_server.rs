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

// ─── Wave 0 RED stubs for LCL-02, LCL-03, LCL-04 ───────────────────────────
// All tests below call routes that do not exist yet. They will receive 404 or
// wrong status codes — that is the expected RED state.

async fn publish_module(client: &reqwest::Client, port: u16, org: &str, name: &str, version: &str) {
    let content = b"module content";
    let form = reqwest::multipart::Form::new().part(
        "module",
        reqwest::multipart::Part::bytes(content.as_ref())
            .file_name("mod.gmod")
            .mime_str("application/octet-stream")
            .unwrap(),
    );
    let resp = client
        .put(format!(
            "http://127.0.0.1:{}/api/v1/modules/{}/{}/{}",
            port, org, name, version
        ))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        200,
        "setup publish must succeed for {}/{}/{}",
        org, name, version
    );
}

#[tokio::test]
async fn test_lifecycle_patch() {
    // LCL-02: valid lifecycle transitions active→deprecated→retired must return 200
    let (port, _tmp) = start_test_server().await;
    let client = reqwest::Client::new();
    publish_module(&client, port, "lcl02org", "lcl02mod", "1.0.0").await;

    let resp = client
        .patch(format!(
            "http://127.0.0.1:{}/api/v1/modules/lcl02org/lcl02mod/1.0.0/lifecycle",
            port
        ))
        .json(&serde_json::json!({"lifecycle": "deprecated"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        200,
        "PATCH to deprecated must return 200"
    );

    let resp = client
        .patch(format!(
            "http://127.0.0.1:{}/api/v1/modules/lcl02org/lcl02mod/1.0.0/lifecycle",
            port
        ))
        .json(&serde_json::json!({"lifecycle": "retired"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        200,
        "PATCH to retired must return 200"
    );
}

#[tokio::test]
async fn test_lifecycle_patch_retired_terminal() {
    // LCL-02: retired is terminal — attempting to transition away must return 422
    let (port, _tmp) = start_test_server().await;
    let client = reqwest::Client::new();
    publish_module(&client, port, "termorg", "termmod", "1.0.0").await;

    let patch_url = format!(
        "http://127.0.0.1:{}/api/v1/modules/termorg/termmod/1.0.0/lifecycle",
        port
    );

    let resp = client
        .patch(&patch_url)
        .json(&serde_json::json!({"lifecycle": "deprecated"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "PATCH to deprecated must return 200");

    let resp = client
        .patch(&patch_url)
        .json(&serde_json::json!({"lifecycle": "retired"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "PATCH to retired must return 200");

    let resp = client
        .patch(&patch_url)
        .json(&serde_json::json!({"lifecycle": "deprecated"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        422,
        "PATCH away from retired must return 422 (terminal state)"
    );
}

#[tokio::test]
async fn test_lifecycle_patch_direct_skip_rejected() {
    // LCL-02: skipping deprecated and going directly to retired must return 422
    let (port, _tmp) = start_test_server().await;
    let client = reqwest::Client::new();
    publish_module(&client, port, "skiporg", "skipmod", "1.0.0").await;

    let resp = client
        .patch(format!(
            "http://127.0.0.1:{}/api/v1/modules/skiporg/skipmod/1.0.0/lifecycle",
            port
        ))
        .json(&serde_json::json!({"lifecycle": "retired"}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        422,
        "direct transition active→retired (skipping deprecated) must return 422"
    );
}

#[tokio::test]
async fn test_content_retired_returns_410() {
    // LCL-03: GET content for a retired module must return 410 Gone
    let (port, _tmp) = start_test_server().await;
    let client = reqwest::Client::new();
    publish_module(&client, port, "retiredgate", "mod", "1.0.0").await;

    let patch_url = format!(
        "http://127.0.0.1:{}/api/v1/modules/retiredgate/mod/1.0.0/lifecycle",
        port
    );
    client
        .patch(&patch_url)
        .json(&serde_json::json!({"lifecycle": "deprecated"}))
        .send()
        .await
        .unwrap();
    client
        .patch(&patch_url)
        .json(&serde_json::json!({"lifecycle": "retired"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/modules/retiredgate/mod/1.0.0/content",
            port
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        410,
        "GET content for retired module must return 410 Gone"
    );
}

#[tokio::test]
async fn test_versions_list_sorted() {
    // LCL-04: GET /versions returns all versions sorted by SemVer ascending
    let (port, _tmp) = start_test_server().await;
    let client = reqwest::Client::new();
    // Publish in non-sorted order
    publish_module(&client, port, "versorg", "vmod", "2.0.0").await;
    publish_module(&client, port, "versorg", "vmod", "1.0.0").await;
    publish_module(&client, port, "versorg", "vmod", "1.5.0").await;

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/modules/versorg/vmod/versions",
            port
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "GET /versions must return 200");

    let body: serde_json::Value = resp.json().await.unwrap();
    let arr = body.as_array().expect("response must be a JSON array");
    assert_eq!(arr.len(), 3, "must list all 3 versions");
    assert_eq!(arr[0]["version"].as_str(), Some("1.0.0"), "first must be 1.0.0");
    assert_eq!(arr[1]["version"].as_str(), Some("1.5.0"), "second must be 1.5.0");
    assert_eq!(arr[2]["version"].as_str(), Some("2.0.0"), "third must be 2.0.0");
    for entry in arr {
        assert_eq!(
            entry["lifecycle"].as_str(),
            Some("active"),
            "lifecycle must be 'active'"
        );
        assert!(
            entry["published_at"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
            "published_at must be a non-empty string"
        );
    }
}

#[tokio::test]
async fn test_versions_list_empty() {
    // LCL-04: GET /versions for unknown coordinate returns 200 with empty array
    let (port, _tmp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/modules/noorg/nomod/versions",
            port
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "GET /versions must return 200 even when empty");

    let body: serde_json::Value = resp.json().await.unwrap();
    let arr = body.as_array().expect("response must be a JSON array");
    assert!(arr.is_empty(), "versions array must be empty for unknown module");
}
