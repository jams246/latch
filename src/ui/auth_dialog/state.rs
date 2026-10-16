use latch::ssh::{self, Prompt};

pub(in crate::ui) fn is_current(prompt: &Prompt, state: ssh::State) -> bool {
    matches!(
        state,
        ssh::State::Connecting | ssh::State::Authenticating | ssh::State::Reconnecting
    ) && match prompt {
        Prompt::HostKey { reply, .. } => !reply.is_closed(),
        Prompt::Secret { reply, .. } => !reply.is_closed(),
        Prompt::KeyboardInteractive { reply, .. } => !reply.is_closed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use latch::ssh::SecretKind;
    use tokio::sync::oneshot;

    #[test]
    fn only_live_pending_attempts_can_show_credentials() {
        let (reply, receiver) = oneshot::channel();
        let prompt = Prompt::Secret {
            kind: SecretKind::Password,
            error: None,
            reply,
        };
        for state in [
            ssh::State::Connecting,
            ssh::State::Authenticating,
            ssh::State::Reconnecting,
        ] {
            assert!(is_current(&prompt, state));
        }
        for state in [
            ssh::State::Connected,
            ssh::State::Disconnected,
            ssh::State::Failed,
        ] {
            assert!(!is_current(&prompt, state));
        }
        drop(receiver);
        assert!(!is_current(&prompt, ssh::State::Authenticating));
    }

    #[test]
    fn closed_trust_and_challenge_replies_are_discarded() {
        let (reply, receiver) = oneshot::channel();
        drop(receiver);
        let trust = Prompt::HostKey {
            host: "127.0.0.1".into(),
            port: 22,
            algorithm: "ssh-ed25519".into(),
            fingerprint: "synthetic".into(),
            reply,
        };
        assert!(!is_current(&trust, ssh::State::Connecting));
        let (reply, receiver) = oneshot::channel();
        drop(receiver);
        let challenge = Prompt::KeyboardInteractive {
            name: "Test".into(),
            instructions: String::new(),
            fields: vec![],
            reply,
        };
        assert!(!is_current(&challenge, ssh::State::Authenticating));
    }
}
