use super::super::MaestroLocal;
use crate::models::ResourceType;
use crate::QuantumResource;

#[tokio::test]
async fn resource_id_and_type_match_backend() {
    const BACKEND_NAME: &str = "maestro_local";

    let mut qrmi = MaestroLocal {
        backend_name: BACKEND_NAME.to_string(),
        session_id: None,
    };

    let resource_id = qrmi
        .resource_id()
        .await
        .expect("resource_id should succeed");
    let resource_type = qrmi
        .resource_type()
        .await
        .expect("resource_type should succeed");

    assert_eq!(resource_id, BACKEND_NAME);
    assert_eq!(resource_type, ResourceType::MaestroLocal);
}

#[tokio::test]
async fn is_accessible_returns_true() {
    let mut qrmi = MaestroLocal {
        backend_name: "maestro_local".to_string(),
        session_id: None,
    };
    let accessible = qrmi
        .is_accessible()
        .await
        .expect("is_accessible should succeed");
    assert!(accessible);
}

#[tokio::test]
async fn acquire_release_session() {
    let mut qrmi = MaestroLocal {
        backend_name: "maestro_local".to_string(),
        session_id: None,
    };
    let session_id = qrmi.acquire().await.expect("acquire should succeed");
    assert!(!session_id.is_empty());
    qrmi.release(&session_id)
        .await
        .expect("release should succeed");
}
