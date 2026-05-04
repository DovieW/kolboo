use crate::licensing::{LicenseStatus, LicenseTier};
use crate::pipeline::{resolve_provider_mode, ProviderMode};

#[test]
fn managed_mode_characterization_covers_personal_and_enterprise_adapters() {
    assert_eq!(
        resolve_provider_mode(
            true,
            LicenseTier::Personal,
            LicenseStatus::Active,
            None,
            None,
            None,
        ),
        ProviderMode::Managed
    );
    assert_eq!(
        resolve_provider_mode(
            true,
            LicenseTier::Enterprise,
            LicenseStatus::Active,
            Some("cloud"),
            Some(true),
            Some(true),
        ),
        ProviderMode::Managed
    );
}

#[test]
fn managed_mode_characterization_preserves_byok_fallbacks() {
    assert_eq!(
        resolve_provider_mode(
            false,
            LicenseTier::Personal,
            LicenseStatus::Active,
            None,
            None,
            None,
        ),
        ProviderMode::Byok
    );
    assert_eq!(
        resolve_provider_mode(
            true,
            LicenseTier::Enterprise,
            LicenseStatus::Active,
            Some("none"),
            Some(true),
            Some(true),
        ),
        ProviderMode::Byok
    );
    assert_eq!(
        resolve_provider_mode(
            true,
            LicenseTier::Personal,
            LicenseStatus::SignedOut,
            None,
            None,
            None,
        ),
        ProviderMode::Byok
    );
}
