pub const SDK_NAME: &str = "fin-sdk";
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn sdk_banner() -> String {
    format!("{SDK_NAME} v{SDK_VERSION}")
}
