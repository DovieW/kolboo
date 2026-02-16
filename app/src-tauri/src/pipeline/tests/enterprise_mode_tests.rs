use crate::licensing::{LicenseStatus, LicenseTier};
use crate::pipeline::config::{resolve_provider_mode, ProviderMode};

#[test]
fn enterprise_routes_to_managed_only_with_valid_eligible_non_none_policy() {
    let mode = resolve_provider_mode(
        LicenseTier::Enterprise,
        LicenseStatus::Active,
        "kolboo_cloud",
        Some("kolboo_cloud"),
        Some("cloud"),
        Some(true),
        Some(true),
    );

    assert_eq!(mode, ProviderMode::Managed);
}

#[test]
fn enterprise_falls_back_to_byok_when_policy_missing_or_invalid() {
    let source_none = resolve_provider_mode(
        LicenseTier::Enterprise,
        LicenseStatus::Active,
        "kolboo_cloud",
        Some("kolboo_cloud"),
        Some("none"),
        Some(true),
        Some(true),
    );
    assert_eq!(source_none, ProviderMode::Byok);

    let invalid = resolve_provider_mode(
        LicenseTier::Enterprise,
        LicenseStatus::Active,
        "kolboo_cloud",
        Some("kolboo_cloud"),
        Some("cloud"),
        Some(true),
        Some(false),
    );
    assert_eq!(invalid, ProviderMode::Byok);

    let ineligible = resolve_provider_mode(
        LicenseTier::Enterprise,
        LicenseStatus::Active,
        "kolboo_cloud",
        Some("kolboo_cloud"),
        Some("cloud"),
        Some(false),
        Some(true),
    );
    assert_eq!(ineligible, ProviderMode::Byok);
}
