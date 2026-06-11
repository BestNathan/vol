use std::io::Write;

use vol_agent_server::{
    config::ServerConfig,
    routes::{ws_owner, WsOwner},
};

#[test]
fn standalone_data_plane_routes_ws_to_data_plane() {
    assert_eq!(ws_owner(false, true).unwrap(), WsOwner::DataPlane);
}

#[test]
fn control_plane_routes_ws_to_control_plane() {
    assert_eq!(ws_owner(true, false).unwrap(), WsOwner::ControlPlane);
    assert_eq!(ws_owner(true, true).unwrap(), WsOwner::ControlPlane);
}

#[test]
fn both_roles_disabled_rejected_by_config() {
    let mut config_file = tempfile::NamedTempFile::new().unwrap();
    write!(
        config_file,
        r#"
[server.roles]
control_plane = false
data_plane = false
"#
    )
    .unwrap();

    let err = ServerConfig::load(config_file.path()).unwrap_err();
    assert!(
        err.contains("at least one server role must be enabled"),
        "unexpected validation error: {err}"
    );
}
