mod key;

use russh::client;
use tokio::sync::{mpsc, oneshot};

use super::{
    AuthMethod, ChallengeField, ConnectionConfig, Error, Prompt, Secret, SecretKind, host::Client,
};

pub(super) async fn authenticate(
    handle: &mut client::Handle<Client>,
    config: &ConnectionConfig,
    prompts: &mpsc::Sender<Prompt>,
    cached: &mut Option<Secret>,
) -> Result<(), Error> {
    match &config.auth {
        AuthMethod::Password => password(handle, &config.username, prompts, cached).await,
        AuthMethod::KeyFile(path) => {
            key::authenticate(handle, &config.username, path, prompts, cached).await
        }
        AuthMethod::Agent => super::agent::authenticate(handle, &config.username).await,
        AuthMethod::KeyboardInteractive => keyboard(handle, &config.username, prompts).await,
    }
}

async fn password(
    handle: &mut client::Handle<Client>,
    username: &str,
    prompts: &mpsc::Sender<Prompt>,
    cached: &mut Option<Secret>,
) -> Result<(), Error> {
    if let Some(secret) = cached.as_ref() {
        return if handle
            .authenticate_password(username, secret.as_str())
            .await?
            .success()
        {
            Ok(())
        } else {
            Err(Error::Authentication)
        };
    }
    let mut error = None;
    loop {
        let (reply, response) = oneshot::channel();
        let secret = super::prompt(
            prompts,
            Prompt::Secret {
                kind: SecretKind::Password,
                error,
                reply,
            },
            response,
        )
        .await?
        .ok_or(Error::Cancelled)?;
        let result = handle
            .authenticate_password(username, secret.as_str())
            .await?;
        if result.success() {
            *cached = Some(secret);
            return Ok(());
        }
        error = Some("Password rejected. Enter it again to retry.".into());
    }
}

async fn keyboard(
    handle: &mut client::Handle<Client>,
    username: &str,
    prompts: &mpsc::Sender<Prompt>,
) -> Result<(), Error> {
    use client::KeyboardInteractiveAuthResponse as Response;
    let mut response = handle
        .authenticate_keyboard_interactive_start(username, None)
        .await?;
    for _ in 0..16 {
        match response {
            Response::Success => return Ok(()),
            Response::Failure { .. } => return Err(Error::Authentication),
            Response::InfoRequest {
                name,
                instructions,
                prompts: fields,
            } => {
                if name.len() > 4096
                    || instructions.len() > 4096
                    || fields.len() > 16
                    || fields.iter().any(|field| field.prompt.len() > 4096)
                {
                    return Err(Error::Challenge);
                }
                let count = fields.len();
                let (reply, receiver) = oneshot::channel();
                let values = super::prompt(
                    prompts,
                    Prompt::KeyboardInteractive {
                        name,
                        instructions,
                        fields: fields
                            .into_iter()
                            .map(|field| ChallengeField {
                                label: field.prompt,
                                echo: field.echo,
                            })
                            .collect(),
                        reply,
                    },
                    receiver,
                )
                .await?
                .ok_or(Error::Cancelled)?;
                if values.len() != count {
                    return Err(Error::Challenge);
                }
                // russh owns its authentication response copies until transmission finishes.
                let answers = values.iter().map(|value| value.to_string()).collect();
                response = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?;
            }
        }
    }
    Err(Error::Challenge)
}
