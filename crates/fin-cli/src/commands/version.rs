use serde_json::json;

use fin_sdk::sdk_banner;

use crate::commands::CommandResult;
use crate::envelope::MetaExtras;
use crate::error::ExitCode;

pub fn run() -> CommandResult {
    let banner = sdk_banner();
    CommandResult {
        tool: "version",
        data: json!({
            "tool": "version",
            "sdk": banner,
        }),
        text: banner,
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    }
}
