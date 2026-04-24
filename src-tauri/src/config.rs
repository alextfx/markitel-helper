//! App-wide constants. Anything tunable via env var lives here.

pub const HELPER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const KEYCHAIN_SERVICE: &str = "com.markitel.helper";
pub const KEYCHAIN_USER_ACCOUNT: &str = "markitel-bridge-key";
pub const DEEP_LINK_SCHEME: &str = "markitel";

/// Base URL of the Markitel backend. Overridable via `MARKITEL_API_BASE`
/// for local dev — defaults to the prod origin.
pub fn api_base() -> String {
    std::env::var("MARKITEL_API_BASE")
        .unwrap_or_else(|_| "https://markitel.com".to_string())
}
