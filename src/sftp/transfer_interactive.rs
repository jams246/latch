use russh_sftp::client::SftpSession;
use tokio::sync::watch;
use uuid::Uuid;

use super::{
    ConflictChoice, Direction, Error,
    transfer::wait_running,
    transfer_conflicts::Resolver,
    transfer_destination::{create_directory, duplicate, existing, rebase_children, renamed},
    transfer_plan::{Node, plan, total_size},
    transfer_progress::Reporter,
    transfer_publish::{self, Replacement},
    transfer_trees::copy_file,
};

pub(super) async fn copy_many(
    session: &SftpSession,
    directions: Vec<Direction>,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
    progress: &mut Reporter,
    resolver: &mut Resolver,
) -> Result<(), Error> {
    let nodes = plan(session, directions, cancel, paused).await?;
    let total = total_size(&nodes);
    progress.total(total);
    let mut context = Context {
        session,
        cancel,
        paused,
        progress,
        resolver,
        total,
    };
    for node in nodes {
        context.copy_node(node, false).await?;
    }
    Ok(())
}

struct Context<'a> {
    session: &'a SftpSession,
    cancel: &'a mut watch::Receiver<bool>,
    paused: &'a mut watch::Receiver<bool>,
    progress: &'a mut Reporter,
    resolver: &'a mut Resolver,
    total: Option<u64>,
}

impl Context<'_> {
    async fn copy_node(&mut self, mut node: Node, merging: bool) -> Result<(), Error> {
        let is_dir = node.children.is_some();
        let mut replacement = None;
        let mut merge_children = merging;
        loop {
            wait_running(self.cancel, self.paused).await?;
            let Some(destination_is_dir) = existing(self.session, &node.direction).await? else {
                if is_dir && let Err(error) = create_directory(self.session, &node.direction).await
                {
                    if existing(self.session, &node.direction).await?.is_some() {
                        continue;
                    }
                    return Err(error);
                }
                break;
            };
            let can_merge = is_dir && destination_is_dir;
            let (choice, approved) = if merging {
                (
                    if can_merge {
                        ConflictChoice::Merge
                    } else {
                        ConflictChoice::Duplicate
                    },
                    None,
                )
            } else {
                let approved = transfer_publish::capture(
                    self.session,
                    &node.direction,
                    self.cancel,
                    self.paused,
                )
                .await?;
                (
                    self.resolver
                        .choose(&node.direction, can_merge, self.cancel)
                        .await?,
                    Some(approved),
                )
            };
            match choice {
                ConflictChoice::Skip => {
                    self.total = self
                        .total
                        .zip(node.total)
                        .and_then(|(total, skipped)| total.checked_sub(skipped));
                    self.progress.total(self.total);
                    return Ok(());
                }
                ConflictChoice::Replace => {
                    replacement = approved;
                    break;
                }
                ConflictChoice::Duplicate => {
                    node.direction = duplicate(
                        self.session,
                        &node.direction,
                        is_dir,
                        self.cancel,
                        self.paused,
                    )
                    .await?;
                }
                ConflictChoice::Merge => {
                    if existing(self.session, &node.direction).await? != Some(true) {
                        continue;
                    }
                    merge_children = true;
                    break;
                }
            }
        }
        wait_running(self.cancel, self.paused).await?;
        if let Some(mut children) = node.children {
            let staged = if replacement.is_some() {
                let staged = renamed(&node.direction, &format!(".latch-{}.part", Uuid::new_v4()))?;
                create_directory(self.session, &staged).await?;
                staged
            } else {
                node.direction
            };
            rebase_children(&mut children, &staged)?;
            let result = self.copy_children(children, merge_children).await;
            self.finish_directory(&staged, replacement.as_ref(), result)
                .await
        } else {
            copy_file(
                self.session,
                &node.direction,
                self.cancel,
                self.paused,
                self.progress,
                replacement.as_ref(),
            )
            .await
        }
    }

    async fn copy_children(&mut self, children: Vec<Node>, merging: bool) -> Result<(), Error> {
        for child in children {
            Box::pin(self.copy_node(child, merging)).await?;
        }
        Ok(())
    }

    async fn finish_directory(
        &mut self,
        staged: &Direction,
        replacement: Option<&Replacement>,
        result: Result<(), Error>,
    ) -> Result<(), Error> {
        let Some(replacement) = replacement else {
            return result;
        };
        let result = match result {
            Ok(()) => wait_running(self.cancel, self.paused).await,
            error => error,
        };
        let result = match result {
            Ok(()) => {
                transfer_publish::publish(
                    self.session,
                    staged,
                    replacement,
                    self.cancel,
                    self.paused,
                )
                .await
            }
            error => error,
        };
        match result {
            Ok(()) => Ok(()),
            Err(Error::Cancelled | Error::PartialRetained { .. }) => Err(Error::PartialRetained {
                path: match staged {
                    Direction::Upload { remote, .. } => remote.clone(),
                    Direction::Download { local, .. } => local.to_string_lossy().into_owned(),
                },
                reason: "The existing destination was preserved; copied data remains in this partial folder.".into(),
            }),
            Err(error @ Error::ReplacementRetained { .. }) => Err(error),
            Err(error) => {
                if existing(self.session, staged).await?.is_some() {
                    transfer_publish::remove(self.session, staged).await?;
                }
                Err(error)
            }
        }
    }
}
