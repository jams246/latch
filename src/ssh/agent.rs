use russh::{client, keys::agent::client::AgentClient};

use super::{Error, host::Client};

pub(super) async fn authenticate(
    handle: &mut client::Handle<Client>,
    username: &str,
) -> Result<(), Error> {
    let agent = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
        .await
        .map_err(|_| Error::Agent)?;
    authenticate_with(handle, username, agent).await
}

pub(super) async fn authenticate_with<S>(
    handle: &mut client::Handle<Client>,
    username: &str,
    mut agent: AgentClient<S>,
) -> Result<(), Error>
where
    S: russh::keys::agent::client::AgentStream + Send + Unpin,
{
    let identities = agent.request_identities().await.map_err(|_| Error::Agent)?;
    for identity in identities {
        let russh::keys::agent::AgentIdentity::PublicKey { key, .. } = identity else {
            continue;
        };
        let hash = handle.best_supported_rsa_hash().await?.flatten();
        if handle
            .authenticate_publickey_with(username, key, hash, &mut agent)
            .await
            .map_err(|_| Error::Agent)?
            .success()
        {
            return Ok(());
        }
    }
    Err(Error::Agent)
}
