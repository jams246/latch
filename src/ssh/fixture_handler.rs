use super::fixture::{Counts, Subsystem, key};
use russh::{Channel, ChannelId, Pty, keys::PublicKey, server};
use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

pub(super) struct Handler {
    pub(super) counts: Arc<Counts>,
    pub(super) reject_shell: bool,
    pub(super) reject_pty: bool,
    pub(super) channels: HashMap<ChannelId, Channel<server::Msg>>,
    pub(super) subsystem: Option<Subsystem>,
    pub(super) flood_output: bool,
}

impl Drop for Handler {
    fn drop(&mut self) {
        self.counts.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl server::Handler for Handler {
    type Error = russh::Error;

    async fn agent_request(
        &mut self,
        _: ChannelId,
        _: &mut server::Session,
    ) -> Result<bool, Self::Error> {
        self.counts.agent_forwarding.fetch_add(1, Ordering::SeqCst);
        Ok(false)
    }

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        self.counts.passwords.fetch_add(1, Ordering::SeqCst);
        self.flood_output = user == "synthetic-output-flood";
        Ok(
            if (user == "synthetic" || self.flood_output) && password == "test-only-password" {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public: &PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        let ppk = russh::keys::parse_public_key_base64(
            "AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti",
        )
        .unwrap();
        Ok(
            if user == "synthetic"
                && (public.key_data() == key(2).public_key().key_data()
                    || public.key_data() == ppk.key_data())
            {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        user: &str,
        _: &str,
        response: Option<server::Response<'a>>,
    ) -> Result<server::Auth, Self::Error> {
        if let Some(mut response) = response {
            self.counts
                .challenge_responses
                .fetch_add(1, Ordering::SeqCst);
            return Ok(
                if response.next().is_some_and(|value| value == b"123456"[..]) {
                    server::Auth::Accept
                } else {
                    server::Auth::reject()
                },
            );
        }
        let prompts = match user {
            "synthetic-oversized-challenge" => vec![(Cow::Borrowed("Code"), false); 17],
            "synthetic-long-challenge" => vec![(Cow::Owned("x".repeat(4097)), false)],
            _ => vec![(Cow::Borrowed("Code"), false)],
        };
        Ok(server::Auth::Partial {
            name: if user == "synthetic-long-name" {
                Cow::Owned("x".repeat(4097))
            } else {
                Cow::Borrowed("Synthetic verification")
            },
            instructions: if user == "synthetic-long-instructions" {
                Cow::Owned("x".repeat(4097))
            } else {
                Cow::Borrowed("Enter the synthetic code")
            },
            prompts: Cow::Owned(prompts),
        })
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.counts.subsystems.fetch_add(1, Ordering::SeqCst);
        if name == "sftp"
            && let Some(subsystem) = self.subsystem
            && let Some(channel_stream) = self.channels.remove(&channel)
        {
            session.channel_success(channel)?;
            subsystem(channel_stream.into_stream());
            return Ok(());
        }
        session.channel_failure(channel)?;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        cols: u32,
        rows: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if cols > 0 && rows > 0 {
            self.counts.ptys.fetch_add(1, Ordering::SeqCst);
        }
        if self.reject_pty {
            session.channel_failure(channel)?;
        } else {
            session.channel_success(channel)?;
        }
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.counts.shells.fetch_add(1, Ordering::SeqCst);
        if self.flood_output {
            session.data(channel, vec![b'x'; 1024 * 1024 + 1])?;
        } else if self.reject_shell {
            session.channel_failure(channel)?;
        } else {
            session.channel_success(channel)?;
            session.data(channel, b"\x1b[32mSynthetic SSH shell\x1b[0m\r\n".to_vec())?;
        }
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if self.channels.contains_key(&channel) {
            session.data(channel, data.to_vec())?;
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.counts.resizes.fetch_add(1, Ordering::SeqCst);
        session.data(channel, b"resized".to_vec())?;
        Ok(())
    }
}
