use std::time::Duration;

use russh::{ChannelMsg, client};

use super::shell::Shell;
use crate::ssh::host::Client;

const REQUEST: &str = "printf '\\036LATCH_SHELL\\037%s\\036' \"$SHELL\"";
const MARKER: &[u8] = b"\x1eLATCH_SHELL\x1f";

pub(super) async fn detect(handle: &client::Handle<Client>) -> Option<Shell> {
    let mut channel = tokio::time::timeout(Duration::from_secs(2), handle.channel_open_session())
        .await
        .ok()?
        .ok()?;
    let result = tokio::time::timeout(Duration::from_secs(2), async {
        channel.exec(true, REQUEST).await.ok()?;
        let mut output = Vec::new();
        let mut remaining = 4096_usize;
        let mut success = false;
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => {
                    remaining = remaining.checked_sub(data.len())?;
                    output.extend_from_slice(&data);
                }
                ChannelMsg::ExtendedData { data, .. } => {
                    remaining = remaining.checked_sub(data.len())?;
                }
                ChannelMsg::ExitStatus { exit_status } => success = exit_status == 0,
                ChannelMsg::Failure => return None,
                ChannelMsg::Close => break,
                _ => (),
            }
        }
        success.then(|| parse(&output)).flatten()
    })
    .await
    .ok()
    .flatten();
    let _ = tokio::time::timeout(Duration::from_millis(200), channel.close()).await;
    result
}

fn parse(output: &[u8]) -> Option<Shell> {
    let start = output
        .windows(MARKER.len())
        .position(|bytes| bytes == MARKER)?
        + MARKER.len();
    let path = output[start..].split(|byte| *byte == 0x1e).next()?;
    if output.get(start + path.len()) != Some(&0x1e) {
        return None;
    }
    Shell::from_path(std::str::from_utf8(path).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_requires_complete_framing_and_supported_absolute_shell() {
        assert!(parse(b"profile output\n\x1eLATCH_SHELL\x1f/bin/bash\x1e").is_some());
        for output in [
            b"/bin/bash".as_slice(),
            b"\x1eLATCH_SHELL\x1f/bin/bash",
            b"\x1eLATCH_SHELL\x1fbash\x1e",
            b"\x1eLATCH_SHELL\x1f/bin/unknown\x1e",
            b"\x1eLATCH_SHELL\x1f/bin/bash\n\x1e",
        ] {
            assert!(parse(output).is_none());
        }
    }
}
