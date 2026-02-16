use crate::licensing::{LicenseStatus, LicenseTier};
use crate::pipeline::config::{resolve_provider_mode, ProviderMode};

#[test]
fn personal_active_routes_to_managed_when_cloud_provider_selected() {
    let mode = resolve_provider_mode(
        LicenseTier::Personal,
        LicenseStatus::Active,
        "kolboo_cloud",
        Some("kolboo_cloud"),
        None,
        None,
        None,
    );

    assert_eq!(mode, ProviderMode::Managed);
}

#[test]
fn personal_signed_out_does_not_route_to_managed() {
    let mode = resolve_provider_mode(
        LicenseTier::Personal,
        LicenseStatus::SignedOut,
        "kolboo_cloud",
        Some("kolboo_cloud"),
        None,
        None,
        None,
    );

    assert_eq!(mode, ProviderMode::Byok);
}
