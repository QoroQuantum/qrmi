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

// Regression coverage for the shared resource factory introduced in QRMI 0.24.
#[tokio::test]
async fn configured_maestro_resource_uses_shared_factory() {
    let resource_type: ResourceType = serde_json::from_str("\"maestro-local\"").unwrap();
    assert_eq!(resource_type, ResourceType::MaestroLocal);
    assert_eq!(resource_type.as_str(), "maestro-local");
    let mut resource = crate::common::create_resource(&resource_type, "sync_test_maestro").unwrap();
    assert_eq!(resource.resource_id().await.unwrap(), "sync_test_maestro");
    assert_eq!(
        resource.resource_type().await.unwrap(),
        ResourceType::MaestroLocal
    );
}
