use std::path::PathBuf;

use crate::{
    event::{
        CommandPayload,
        emit::{emit_command, emit_warning},
    },
    util::io,
};
use url::form_urlencoded;
use urlencoding::decode;

/// Entry points that would create an instance or install content outside
/// the instances published by the club server are refused with this text.
const MANAGED_INSTANCES_ONLY: &str =
    "This launcher only installs the instances published by the club server";

/// Refuses an external entry point that manages an instance the club
/// server did not publish.
async fn reject_unmanaged_entry() -> crate::Result<CommandPayload> {
    let _ = emit_warning(MANAGED_INSTANCES_ONLY).await;
    Err(crate::ErrorKind::InputError(MANAGED_INSTANCES_ONLY.to_string()).into())
}

/// Handles external functions (such as through URL deep linkage)
/// Link is extracted value (link) in somewhat URL format, such as
/// subdomain1/subdomain2
/// (Does not include axolotl://)
pub async fn handle_url(sublink: &str) -> crate::Result<CommandPayload> {
    if sublink == "discovery" {
        return reject_unmanaged_entry().await;
    }

    if let Some(query) = sublink.strip_prefix("launch?") {
        let instance_id = form_urlencoded::parse(query.as_bytes()).find_map(
            |(key, value)| (key == "instance_id").then(|| value.into_owned()),
        );
        if let Some(id) = instance_id.filter(|id| !id.is_empty()) {
            return Ok(CommandPayload::LaunchInstance {
                id,
                server: None,
                singleplayer_world: None,
            });
        }
        return Err(crate::ErrorKind::InputError(
            "Launch command requires an instance_id query parameter"
                .to_string(),
        )
        .into());
    }

    // /seed-map?{query}   -    Used to open the Lab seed map
    if let Some(rest) = sublink.strip_prefix("seed-map")
        && (rest.is_empty() || rest.starts_with('?') || rest.starts_with('/'))
    {
        return reject_unmanaged_entry().await;
    }
    Ok(match sublink.split_once('/') {
        // /mod/{id}   -    Installs a mod of mod id
        Some(("mod", _)) => {
            return reject_unmanaged_entry().await;
        }
        // /version/{id}   -    Installs a specific version of id
        Some(("version", _)) => {
            return reject_unmanaged_entry().await;
        }
        // /modpack/{id}   -    Installs a modpack of modpack id
        Some(("modpack", _)) => {
            return reject_unmanaged_entry().await;
        }
        // /server/{id}   -    Opens a server project page and triggers play flow
        Some(("server", _)) => {
            return reject_unmanaged_entry().await;
        }
        // /launch/instance/{id}   -    Launches an instance
        Some(("launch", rest)) if rest.starts_with("instance/") => {
            let raw = rest.trim_start_matches("instance/");
            let (raw, query) = raw.split_once('?').unwrap_or((raw, ""));
            let mut server = None;
            let mut singleplayer_world = None;

            for (key, value) in form_urlencoded::parse(query.as_bytes()) {
                match &*key {
                    "server" => server = Some(value.into_owned()),
                    "singleplayer_world" => {
                        singleplayer_world = Some(value.into_owned());
                    }
                    _ => {}
                }
            }

            if server.is_some() && singleplayer_world.is_some() {
                emit_warning(
                    "Invalid command, cannot launch both a server and a singleplayer world",
                )
                .await?;
                return Err(crate::ErrorKind::InputError(
                    "Cannot launch both a server and a singleplayer world"
                        .to_string(),
                )
                .into());
            }

            match decode(raw) {
                Ok(decoded) => CommandPayload::LaunchInstance {
                    id: decoded.to_string(),
                    server,
                    singleplayer_world,
                },
                Err(e) => {
                    emit_warning(&format!(
                        "Invalid UTF-8 in instance path: {e}"
                    ))
                    .await?;
                    return Err(crate::ErrorKind::InputError(format!(
                        "Invalid UTF-8 in instance path: {e}"
                    ))
                    .into());
                }
            }
        }
        _ => {
            emit_warning(&format!(
                "Invalid command, unrecognized path: {sublink}"
            ))
            .await?;
            return Err(crate::ErrorKind::InputError(format!(
                "Invalid command, unrecognized path: {sublink}"
            ))
            .into());
        }
    })
}

pub async fn parse_command(
    command_string: &str,
) -> crate::Result<CommandPayload> {
    tracing::debug!("Parsing command: {}", &command_string);

    // axolotl://some-command
    // This occurs when following a web redirect link
    if let Some(sublink) = command_string.strip_prefix("axolotl://") {
        Ok(handle_url(sublink).await?)
    } else {
        // We assume anything else is a filepath to a modpack file; zip
        // archives are format-sniffed by the pack installer. Opening one
        // would create an instance outside the club server, so it is
        // refused here.
        let path = PathBuf::from(command_string);
        let path = io::canonicalize(path)?;
        if let Some(ext) = path.extension()
            && (ext == "mrpack" || ext == "zip")
        {
            return reject_unmanaged_entry().await;
        }
        emit_warning(&format!(
            "Invalid command, unrecognized filetype: {}",
            path.display()
        ))
        .await?;
        Err(crate::ErrorKind::InputError(format!(
            "Invalid command, unrecognized filetype: {}",
            path.display()
        ))
        .into())
    }
}

pub async fn parse_and_emit_command(command_string: &str) -> crate::Result<()> {
    let command = parse_command(command_string).await?;
    emit_command(command).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_launch_query_command() {
        let command =
            parse_command("axolotl://launch?instance_id=example%20instance")
                .await
                .unwrap();
        assert!(matches!(
            command,
            CommandPayload::LaunchInstance { id, server: None, singleplayer_world: None }
                if id == "example instance"
        ));
    }

    #[tokio::test]
    async fn refuses_discovery_command() {
        assert!(parse_command("axolotl://discovery").await.is_err());
    }

    #[tokio::test]
    async fn refuses_content_install_commands() {
        for command in [
            "axolotl://mod/example",
            "axolotl://version/example",
            "axolotl://modpack/example",
            "axolotl://server/example",
            "axolotl://seed-map?seed=1",
        ] {
            assert!(parse_command(command).await.is_err(), "{command}");
        }
    }
}
