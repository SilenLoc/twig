use std::{
    collections::HashSet,
    io::Cursor,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};

use crate::{Error, HookContext, RefUpdate, Request};

const ZERO_ID: &str = "0000000000000000000000000000000000000000";

struct Capabilities {
    report_status: bool,
    sideband: bool,
    delete_refs: bool,
}

struct UpdateCommand {
    old_id: String,
    new_id: String,
    refname: String,
}

struct ParsedRequest<'a> {
    updates: Vec<UpdateCommand>,
    pack: &'a [u8],
    capabilities: Capabilities,
}

struct CommandStatus {
    refname: String,
    error: Option<String>,
}

pub(super) fn handle(repo_path: &Path, request: &Request<'_>) -> Result<Vec<u8>, Error> {
    let parsed = parse_request(request.body)?;
    if parsed.updates.is_empty() {
        return Err(Error::Protocol(
            "receive-pack request contains no ref updates".to_string(),
        ));
    }
    if !parsed.capabilities.report_status {
        return Err(Error::Protocol(
            "receive-pack client did not request report-status".to_string(),
        ));
    }
    if parsed.updates.iter().any(|update| update.new_id == ZERO_ID)
        && !parsed.capabilities.delete_refs
    {
        return Err(Error::Protocol(
            "receive-pack deletion was not negotiated".to_string(),
        ));
    }

    let objects_dir = repo_path.join("objects");
    let quarantine = tempfile::Builder::new()
        .prefix("tmp_objdir-incoming-")
        .tempdir_in(&objects_dir)
        .map_err(Error::Io)?;
    let pack_outcome = import_pack(quarantine.path(), parsed.pack)?;
    let promoted = match pack_outcome {
        Some(outcome) => promote_pack(repo_path, outcome)?,
        None => Vec::new(),
    };

    let updates = public_updates(&parsed.updates);
    if let Some((refname, message)) = validate_protected_updates(
        repo_path,
        &parsed.updates,
        request.may_rewrite_protected_branches,
    )? {
        remove_promoted_files(&promoted);
        let statuses = rejected_statuses(&parsed.updates, Some(&refname), &message);
        return encode_report_status(&parsed.capabilities, &statuses, &message);
    }

    let context = HookContext {
        authenticated_user: request.authenticated_user,
        may_rewrite_protected_branches: request.may_rewrite_protected_branches,
    };
    for hook in request.receive_hooks {
        if let Err(error) = hook.pre_receive(repo_path, &updates, &context) {
            remove_promoted_files(&promoted);
            let statuses = rejected_statuses(&parsed.updates, None, "pre-receive hook declined");
            return encode_report_status(&parsed.capabilities, &statuses, &error.message);
        }
    }
    for (index, update) in updates.iter().enumerate() {
        for hook in request.receive_hooks {
            if let Err(error) = hook.update(repo_path, update, &context) {
                remove_promoted_files(&promoted);
                let statuses = rejected_statuses(
                    &parsed.updates,
                    Some(&parsed.updates[index].refname),
                    "update hook declined",
                );
                return encode_report_status(&parsed.capabilities, &statuses, &error.message);
            }
        }
    }

    if let Err(error) = apply_updates(repo_path, &parsed.updates) {
        remove_promoted_files(&promoted);
        return Err(error);
    }
    for hook in request.receive_hooks {
        hook.post_receive(repo_path, &updates, &context);
        let refnames = updates
            .iter()
            .map(|update| update.refname.clone())
            .collect::<Vec<_>>();
        hook.post_update(repo_path, &refnames, &context);
    }
    let statuses = parsed
        .updates
        .iter()
        .map(|update| CommandStatus {
            refname: update.refname.clone(),
            error: None,
        })
        .collect::<Vec<_>>();
    encode_report_status(&parsed.capabilities, &statuses, "")
}

fn parse_request(body: &[u8]) -> Result<ParsedRequest<'_>, Error> {
    let mut updates = Vec::new();
    let mut capabilities = Capabilities {
        report_status: false,
        sideband: false,
        delete_refs: false,
    };
    let mut position = 0;

    loop {
        if position + 4 > body.len() {
            return Err(Error::Protocol(
                "receive-pack request is missing its command flush".to_string(),
            ));
        }
        let length_text = std::str::from_utf8(&body[position..position + 4])
            .map_err(|error| Error::Protocol(error.to_string()))?;
        let length = usize::from_str_radix(length_text, 16)
            .map_err(|error| Error::Protocol(error.to_string()))?;
        if length == 0 {
            position += 4;
            break;
        }
        if length < 4 || position + length > body.len() {
            return Err(Error::Protocol("invalid receive-pack pkt-line".to_string()));
        }

        let payload = &body[position + 4..position + length];
        let (command, capability_bytes) = match payload.iter().position(|byte| *byte == 0) {
            Some(nul) => (&payload[..nul], Some(&payload[nul + 1..])),
            None => (payload, None),
        };
        if let Some(capability_bytes) = capability_bytes {
            for capability in capability_bytes.split(|byte| byte.is_ascii_whitespace()) {
                match capability {
                    b"report-status" | b"report-status-v2" => {
                        capabilities.report_status = true;
                    }
                    b"side-band-64k" | b"side-band" => capabilities.sideband = true,
                    b"delete-refs" => capabilities.delete_refs = true,
                    _ => {}
                }
            }
        }

        let command = std::str::from_utf8(command)
            .map_err(|error| Error::Protocol(error.to_string()))?
            .trim_end_matches('\n');
        let mut fields = command.split_ascii_whitespace();
        let old_id = fields
            .next()
            .ok_or_else(|| Error::Protocol("missing old object id".to_string()))?;
        let new_id = fields
            .next()
            .ok_or_else(|| Error::Protocol("missing new object id".to_string()))?;
        let refname = fields
            .next()
            .ok_or_else(|| Error::Protocol("missing ref name".to_string()))?;
        if fields.next().is_some() {
            return Err(Error::Protocol(
                "too many receive-pack command fields".to_string(),
            ));
        }
        validate_oid(old_id)?;
        validate_oid(new_id)?;
        updates.push(UpdateCommand {
            old_id: old_id.to_string(),
            new_id: new_id.to_string(),
            refname: refname.to_string(),
        });
        position += length;
    }

    Ok(ParsedRequest {
        updates,
        pack: &body[position..],
        capabilities,
    })
}

fn validate_oid(oid: &str) -> Result<(), Error> {
    if oid.len() != 40 || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::Protocol(format!("invalid SHA-1 object id: {oid}")));
    }
    Ok(())
}

fn import_pack(
    quarantine_objects: &Path,
    pack_bytes: &[u8],
) -> Result<Option<gix_pack::bundle::write::Outcome>, Error> {
    if pack_bytes.is_empty() {
        return Ok(None);
    }
    if !pack_bytes.starts_with(b"PACK") {
        return Err(Error::Protocol(
            "receive-pack body has no PACK stream".to_string(),
        ));
    }

    let pack_dir = quarantine_objects.join("pack");
    std::fs::create_dir_all(&pack_dir).map_err(Error::Io)?;
    let mut progress = gix::progress::Discard;
    let mut pack = Cursor::new(pack_bytes);
    gix_pack::Bundle::write_to_directory(
        &mut pack,
        Some(&pack_dir),
        &mut progress,
        &AtomicBool::new(false),
        None::<&gix::Repository>,
        Default::default(),
    )
    .map(Some)
    .map_err(|error| Error::Protocol(format!("incoming pack rejected: {error}")))
}

fn public_updates(updates: &[UpdateCommand]) -> Vec<RefUpdate> {
    updates
        .iter()
        .map(|update| RefUpdate {
            refname: update.refname.clone(),
            old_oid: (update.old_id != ZERO_ID).then(|| update.old_id.clone()),
            new_oid: (update.new_id != ZERO_ID).then(|| update.new_id.clone()),
        })
        .collect()
}

fn rejected_statuses(
    updates: &[UpdateCommand],
    failed_ref: Option<&str>,
    reason: &str,
) -> Vec<CommandStatus> {
    updates
        .iter()
        .map(|update| CommandStatus {
            refname: update.refname.clone(),
            error: Some(
                if failed_ref.is_none_or(|failed| failed == update.refname) {
                    reason.to_string()
                } else {
                    "transaction aborted because another ref update was rejected".to_string()
                },
            ),
        })
        .collect()
}

fn validate_protected_updates(
    repo_path: &Path,
    updates: &[UpdateCommand],
    may_rewrite_protected_branches: bool,
) -> Result<Option<(String, String)>, Error> {
    if may_rewrite_protected_branches {
        return Ok(None);
    }
    let repo = gix::open(repo_path).map_err(|error| Error::Repository(error.to_string()))?;
    for update in updates
        .iter()
        .filter(|update| update.refname == "refs/heads/main")
    {
        if update.new_id == ZERO_ID {
            return Ok(Some((
                update.refname.clone(),
                "Twig: This permission set cannot delete main.".to_string(),
            )));
        }
        if update.old_id != ZERO_ID && update.old_id != update.new_id {
            let old = gix::ObjectId::from_hex(update.old_id.as_bytes())
                .map_err(|error| Error::Protocol(error.to_string()))?;
            let new = gix::ObjectId::from_hex(update.new_id.as_bytes())
                .map_err(|error| Error::Protocol(error.to_string()))?;
            if !is_ancestor(&repo, old, new)? {
                return Ok(Some((
                    update.refname.clone(),
                    "Twig: This permission set cannot rewrite history on main.".to_string(),
                )));
            }
        }
    }
    Ok(None)
}

fn is_ancestor(
    repo: &gix::Repository,
    ancestor: gix::ObjectId,
    descendant: gix::ObjectId,
) -> Result<bool, Error> {
    if ancestor == descendant {
        return Ok(true);
    }
    let mut visited = HashSet::new();
    let mut pending = vec![descendant];
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let object = repo
            .find_object(id)
            .map_err(|error| Error::Repository(error.to_string()))?;
        if object.kind != gix::object::Kind::Commit {
            continue;
        }
        for parent in gix::objs::CommitRefIter::from_bytes(&object.data).parent_ids() {
            if parent == ancestor {
                return Ok(true);
            }
            pending.push(parent);
        }
    }
    Ok(false)
}

fn promote_pack(
    repo_path: &Path,
    outcome: gix_pack::bundle::write::Outcome,
) -> Result<Vec<PathBuf>, Error> {
    let target_dir = repo_path.join("objects/pack");
    std::fs::create_dir_all(&target_dir).map_err(Error::Io)?;
    let mut promoted = Vec::new();
    for source in [outcome.data_path.as_ref(), outcome.index_path.as_ref()]
        .into_iter()
        .flatten()
    {
        if !source.exists() {
            continue;
        }
        let Some(filename) = source.file_name() else {
            return Err(Error::Protocol("pack output has no file name".to_string()));
        };
        let target = target_dir.join(filename);
        if target.exists() {
            std::fs::remove_file(source).map_err(Error::Io)?;
        } else {
            std::fs::rename(source, &target).map_err(Error::Io)?;
            promoted.push(target);
        }
    }
    if let Some(keep_path) = outcome.keep_path {
        let _ = std::fs::remove_file(keep_path);
    }
    Ok(promoted)
}

fn remove_promoted_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
}

fn apply_updates(repo_path: &Path, updates: &[UpdateCommand]) -> Result<(), Error> {
    let repo = gix::open(repo_path).map_err(|error| Error::Repository(error.to_string()))?;
    let mut edits = Vec::with_capacity(updates.len());
    for update in updates {
        let name: gix::refs::FullName = update
            .refname
            .as_str()
            .try_into()
            .map_err(|error| Error::Protocol(format!("invalid ref name: {error}")))?;
        let old = if update.old_id == ZERO_ID {
            None
        } else {
            Some(
                gix::ObjectId::from_hex(update.old_id.as_bytes())
                    .map_err(|error| Error::Protocol(error.to_string()))?,
            )
        };
        let new = if update.new_id == ZERO_ID {
            None
        } else {
            Some(
                gix::ObjectId::from_hex(update.new_id.as_bytes())
                    .map_err(|error| Error::Protocol(error.to_string()))?,
            )
        };
        let expected = match old {
            None => PreviousValue::MustNotExist,
            Some(old) => PreviousValue::MustExistAndMatch(gix::refs::Target::Object(old)),
        };
        let change = match new {
            None => Change::Delete {
                expected,
                log: RefLog::AndReference,
            },
            Some(new) => {
                let object = repo
                    .find_object(new)
                    .map_err(|error| Error::Repository(error.to_string()))?;
                if update.refname.starts_with("refs/heads/")
                    && object.kind != gix::object::Kind::Commit
                {
                    return Err(Error::Protocol(format!(
                        "branch {} must point to a commit",
                        update.refname
                    )));
                }
                Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: "push".into(),
                    },
                    expected,
                    new: gix::refs::Target::Object(new),
                }
            }
        };
        edits.push(RefEdit {
            change,
            name,
            deref: false,
        });
    }
    repo.edit_references(edits)
        .map_err(|error| Error::Repository(error.to_string()))?;
    Ok(())
}

fn encode_report_status(
    capabilities: &Capabilities,
    statuses: &[CommandStatus],
    diagnostic: &str,
) -> Result<Vec<u8>, Error> {
    let mut report = encode_pkt_line(b"unpack ok\n")?;
    for status in statuses {
        let line = match &status.error {
            Some(error) => format!("ng {} {error}\n", status.refname),
            None => format!("ok {}\n", status.refname),
        };
        report.extend(encode_pkt_line(line.as_bytes())?);
    }
    report.extend_from_slice(b"0000");

    if !capabilities.sideband {
        return Ok(report);
    }
    let mut response = Vec::new();
    if !diagnostic.is_empty() {
        let diagnostic = format!("{diagnostic}\n");
        for chunk in diagnostic.as_bytes().chunks(65_515) {
            let mut payload = Vec::with_capacity(chunk.len() + 1);
            payload.push(2);
            payload.extend_from_slice(chunk);
            response.extend(encode_pkt_line(&payload)?);
        }
    }
    for chunk in report.chunks(65_515) {
        let mut payload = Vec::with_capacity(chunk.len() + 1);
        payload.push(1);
        payload.extend_from_slice(chunk);
        response.extend(encode_pkt_line(&payload)?);
    }
    response.extend_from_slice(b"0000");
    Ok(response)
}

fn encode_pkt_line(payload: &[u8]) -> Result<Vec<u8>, Error> {
    let length = payload.len() + 4;
    if length > 0xffff {
        return Err(Error::Protocol("pkt-line exceeds maximum length".into()));
    }
    let mut encoded = format!("{length:04x}").into_bytes();
    encoded.extend_from_slice(payload);
    Ok(encoded)
}
