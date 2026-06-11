use axum::{routing::get, Router};

use crate::health;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsOwner {
    DataPlane,
    ControlPlane,
}

pub fn ws_owner(control_plane: bool, data_plane: bool) -> Result<WsOwner, String> {
    match (control_plane, data_plane) {
        (false, true) => Ok(WsOwner::DataPlane),
        (true, false) | (true, true) => Ok(WsOwner::ControlPlane),
        (false, false) => Err("at least one server role must be enabled".to_string()),
    }
}

pub fn base_router() -> Router {
    Router::new().route("/health", get(health::health))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_plane_owns_ws_only_when_control_plane_disabled() {
        assert_eq!(ws_owner(false, true).unwrap(), WsOwner::DataPlane);
    }

    #[test]
    fn control_plane_owns_ws_when_enabled() {
        assert_eq!(ws_owner(true, false).unwrap(), WsOwner::ControlPlane);
        assert_eq!(ws_owner(true, true).unwrap(), WsOwner::ControlPlane);
    }

    #[test]
    fn both_roles_disabled_is_error() {
        assert!(ws_owner(false, false).is_err());
    }
}
