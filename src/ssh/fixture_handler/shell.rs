use super::Handler;
use crate::ssh::session::shell::Shell;
use russh::{ChannelId, server};
use std::sync::atomic::Ordering;

impl Handler {
    pub(super) fn open_shell(
        &mut self,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), russh::Error> {
        let opened = self.counts.shells.fetch_add(1, Ordering::SeqCst);
        if self.flood_output {
            session.data(channel, vec![b'x'; 1024 * 1024 + 1])?;
        } else if self.reject_shell {
            session.channel_failure(channel)?;
        } else {
            session.channel_success(channel)?;
            session.data(channel, b"\x1b[32mSynthetic SSH shell\x1b[0m\r\n".to_vec())?;
            if self.shell.is_some() {
                if opened == 0
                    && let Some(directory) = self.counts.directory.lock().unwrap().as_deref()
                {
                    self.directory = directory.to_string();
                }
                session.data(channel, osc7(&self.directory))?;
                session.data(
                    channel,
                    format!("Working directory: {}\r\n", self.directory).into_bytes(),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn execute(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), russh::Error> {
        const PROBE: &str = "printf '\\036LATCH_SHELL\\037%s\\036' \"$SHELL\"";
        let command = String::from_utf8_lossy(data);
        self.counts.execs.lock().unwrap().push(command.to_string());
        let Some(shell) = self.shell else {
            session.channel_failure(channel)?;
            session.close(channel)?;
            self.channels.remove(&channel);
            return Ok(());
        };
        if command.as_ref() == PROBE {
            session.channel_success(channel)?;
            session.data(
                channel,
                format!("\x1eLATCH_SHELL\x1f{shell}\x1e").into_bytes(),
            )?;
            session.exit_status_request(channel, 0)?;
            session.eof(channel)?;
            session.close(channel)?;
            self.channels.remove(&channel);
            return Ok(());
        }
        if self.counts.reject_wrapped_shell.load(Ordering::SeqCst) {
            session.channel_failure(channel)?;
            session.close(channel)?;
            self.channels.remove(&channel);
            return Ok(());
        }
        if let Some(directory) = self.counts.directory.lock().unwrap().as_deref()
            && Shell::from_path(shell)
                .is_some_and(|shell| shell.command(Some(directory)) == command.as_ref())
        {
            self.directory = directory.to_string();
        }
        self.open_shell(channel, session)
    }

    pub(super) fn shell_input(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), russh::Error> {
        if self.shell.is_some()
            && let Some(path) = data.strip_prefix(b"synthetic-cwd:")
        {
            self.directory = String::from_utf8_lossy(path).into_owned();
            *self.counts.directory.lock().unwrap() = Some(self.directory.clone());
            session.data(channel, osc7(&self.directory))?;
            return Ok(());
        }
        if data == b"synthetic-drop" {
            session.disconnect(
                russh::Disconnect::ByApplication,
                "Synthetic disconnect",
                "en",
            )?;
            return Ok(());
        }
        if data == b"synthetic-exit" || data == b"synthetic-eof-exit" {
            if data == b"synthetic-eof-exit" {
                session.eof(channel)?;
            }
            session.exit_status_request(channel, 0)?;
            session.close(channel)?;
            return Ok(());
        }
        if data == b"synthetic-shell-close" {
            session.close(channel)?;
            return Ok(());
        }
        if self.channels.contains_key(&channel) {
            session.data(channel, data.to_vec())?;
        }
        Ok(())
    }
}

fn osc7(path: &str) -> Vec<u8> {
    let encoded: String = path
        .bytes()
        .map(|byte| match byte {
            b'/' | b'-' | b'_' | b'.' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect();
    format!("\x1b]7;file://synthetic{encoded}\x1b\\").into_bytes()
}
