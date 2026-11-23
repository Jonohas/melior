//! The image store on a real S3-compatible server (`RustFS`). Needs one, so it is
//! ignored by default:
//!
//! ```sh
//! MELIOR_TEST_S3_ENDPOINT=http://127.0.0.1:9000 MELIOR_TEST_S3_ACCESS_KEY=melior \
//! MELIOR_TEST_S3_SECRET_KEY=... \
//!     cargo test -p melior-infrastructure --test s3_images -- --ignored
//! ```

use melior_application::ImageStore;
use melior_domain::{AttachmentId, ImageFormat};
use melior_infrastructure::s3::sigv4::Credentials;
use melior_infrastructure::s3::{S3ImageStore, S3Settings};
use std::time::{SystemTime, UNIX_EPOCH};

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

/// A store on a bucket no earlier run used, so the first upload creates it.
fn store(secret_key: &str) -> S3ImageStore {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    S3ImageStore::new(S3Settings {
        endpoint: var("MELIOR_TEST_S3_ENDPOINT"),
        bucket: format!("melior-test-{nanos}"),
        credentials: Credentials {
            access_key: var("MELIOR_TEST_S3_ACCESS_KEY"),
            secret_key: secret_key.to_owned(),
            region: "us-east-1".into(),
        },
    })
    .unwrap()
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_S3_ENDPOINT"]
async fn an_image_is_stored_in_a_new_bucket_and_read_back() {
    let store = store(&var("MELIOR_TEST_S3_SECRET_KEY"));
    let id = AttachmentId::generate();
    let bytes = b"\x89PNG\r\n\x1a\nbytes".to_vec();

    store.put(&id, ImageFormat::Png, bytes.clone()).await.unwrap();

    assert_eq!(store.get(&id).await.unwrap(), Some(bytes));
    assert_eq!(store.get(&AttachmentId::generate()).await.unwrap(), None);

    store.delete(&id).await.unwrap();
    assert_eq!(store.get(&id).await.unwrap(), None);
    store.delete(&id).await.unwrap();
}

#[tokio::test]
#[ignore = "needs MELIOR_TEST_S3_ENDPOINT"]
async fn a_wrong_secret_is_refused() {
    let store = store("not-the-secret");

    let result = store.put(&AttachmentId::generate(), ImageFormat::Png, b"x".to_vec()).await;

    assert!(result.is_err());
}
