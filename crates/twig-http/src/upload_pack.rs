use std::{
    collections::{HashSet, VecDeque},
    path::Path,
};

use sha1::{Digest, Sha1};

use crate::Error;

struct UploadRequest {
    wants: Vec<gix::ObjectId>,
    haves: Vec<gix::ObjectId>,
    done: bool,
    sideband: bool,
}

struct PackObject {
    kind: gix::object::Kind,
    data: Vec<u8>,
}

pub(super) fn handle(repo_path: &Path, request_body: &[u8]) -> Result<Vec<u8>, Error> {
    let request = parse_request(request_body)?;
    if request.wants.is_empty() {
        return Err(Error::Protocol(
            "upload-pack request contains no wanted objects".to_string(),
        ));
    }

    let repo = gix::open(repo_path).map_err(|error| Error::Repository(error.to_string()))?;
    let advertised = advertised_object_ids(&repo)?;
    for want in &request.wants {
        if !advertised.contains(want) {
            return Err(Error::Protocol(format!(
                "requested object {want} was not advertised"
            )));
        }
    }

    let mut response = encode_pkt_line(b"NAK\n")?;
    if !request.done {
        response.extend_from_slice(b"0000");
        return Ok(response);
    }

    let objects = collect_wanted_objects(&repo, &request.wants, &request.haves)?;
    let pack = encode_pack(objects)?;
    if request.sideband {
        for chunk in pack.chunks(65_515) {
            let mut payload = Vec::with_capacity(chunk.len() + 1);
            payload.push(1);
            payload.extend_from_slice(chunk);
            response.extend(encode_pkt_line(&payload)?);
        }
        response.extend_from_slice(b"0000");
    } else {
        response.extend(pack);
    }
    Ok(response)
}

fn advertised_object_ids(repo: &gix::Repository) -> Result<HashSet<gix::ObjectId>, Error> {
    let mut ids = HashSet::new();
    if let Ok(head) = repo.head_id() {
        ids.insert(head.detach());
    }
    let references = repo
        .references()
        .map_err(|error| Error::Repository(error.to_string()))?;
    let all_references = references
        .all()
        .map_err(|error| Error::Repository(error.to_string()))?;
    for reference in all_references {
        let reference = reference.map_err(|error| Error::Repository(error.to_string()))?;
        if let Some(id) = reference.try_id() {
            ids.insert(id.detach());
        }
    }
    Ok(ids)
}

fn parse_request(body: &[u8]) -> Result<UploadRequest, Error> {
    let mut request = UploadRequest {
        wants: Vec::new(),
        haves: Vec::new(),
        done: false,
        sideband: false,
    };
    let mut position = 0;

    while position < body.len() {
        if body[position..].starts_with(b"0000") {
            position += 4;
            continue;
        }
        if position + 4 > body.len() {
            return Err(Error::Protocol("truncated pkt-line prefix".to_string()));
        }
        let length_text = std::str::from_utf8(&body[position..position + 4])
            .map_err(|error| Error::Protocol(error.to_string()))?;
        let length = usize::from_str_radix(length_text, 16)
            .map_err(|error| Error::Protocol(error.to_string()))?;
        if length < 4 || position + length > body.len() {
            return Err(Error::Protocol("invalid pkt-line length".to_string()));
        }
        let payload = &body[position + 4..position + length];
        let command = payload.split(|byte| *byte == 0).next().unwrap_or_default();
        let command = std::str::from_utf8(command)
            .map_err(|error| Error::Protocol(error.to_string()))?
            .trim_end_matches('\n');

        if let Some(arguments) = command.strip_prefix("want ") {
            let oid = arguments
                .split_ascii_whitespace()
                .next()
                .ok_or_else(|| Error::Protocol("want packet is missing an object id".into()))?;
            request.wants.push(
                gix::ObjectId::from_hex(oid.as_bytes())
                    .map_err(|error| Error::Protocol(error.to_string()))?,
            );
            request.sideband |= arguments
                .split_ascii_whitespace()
                .any(|capability| capability == "side-band-64k" || capability == "side-band");
            if let Some(nul) = payload.iter().position(|byte| *byte == 0) {
                let capabilities = &payload[nul + 1..];
                request.sideband |= capabilities
                    .split(|byte| byte.is_ascii_whitespace())
                    .any(|capability| capability == b"side-band-64k" || capability == b"side-band");
            }
        } else if let Some(oid) = command.strip_prefix("have ") {
            request.haves.push(
                gix::ObjectId::from_hex(oid.as_bytes())
                    .map_err(|error| Error::Protocol(error.to_string()))?,
            );
        } else if command == "done" {
            request.done = true;
        } else if command.starts_with("shallow ") || command.starts_with("deepen ") {
            return Err(Error::Protocol(
                "shallow upload-pack requests are not supported".to_string(),
            ));
        }
        position += length;
    }
    Ok(request)
}

fn collect_wanted_objects(
    repo: &gix::Repository,
    wants: &[gix::ObjectId],
    haves: &[gix::ObjectId],
) -> Result<Vec<PackObject>, Error> {
    let mut excluded = HashSet::new();
    for have in haves {
        let _ = collect_reachable(repo, *have, &mut excluded);
    }

    let mut seen = HashSet::new();
    let mut pending: VecDeque<_> = wants.iter().copied().collect();
    let mut objects = Vec::new();
    while let Some(id) = pending.pop_front() {
        if excluded.contains(&id) || !seen.insert(id) {
            continue;
        }
        let object = repo
            .find_object(id)
            .map_err(|error| Error::Repository(error.to_string()))?;
        let dependencies = object_dependencies(object.kind, &object.data)?;
        pending.extend(dependencies);
        objects.push(PackObject {
            kind: object.kind,
            data: object.data.clone(),
        });
    }
    Ok(objects)
}

fn collect_reachable(
    repo: &gix::Repository,
    start: gix::ObjectId,
    seen: &mut HashSet<gix::ObjectId>,
) -> Result<(), Error> {
    let mut pending = VecDeque::from([start]);
    while let Some(id) = pending.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        let Ok(object) = repo.find_object(id) else {
            continue;
        };
        let dependencies = object_dependencies(object.kind, &object.data)?;
        pending.extend(dependencies);
    }
    Ok(())
}

fn object_dependencies(kind: gix::object::Kind, data: &[u8]) -> Result<Vec<gix::ObjectId>, Error> {
    let mut dependencies = Vec::new();
    match kind {
        gix::object::Kind::Commit => {
            let tree = gix::objs::CommitRefIter::from_bytes(data)
                .tree_id()
                .map_err(|error| Error::Protocol(error.to_string()))?;
            dependencies.push(tree);
            dependencies.extend(gix::objs::CommitRefIter::from_bytes(data).parent_ids());
        }
        gix::object::Kind::Tree => {
            for entry in gix::objs::TreeRefIter::from_bytes(data) {
                let entry = entry.map_err(|error| Error::Protocol(error.to_string()))?;
                if !entry.mode.is_commit() {
                    dependencies.push(entry.oid.to_owned());
                }
            }
        }
        gix::object::Kind::Tag => {
            let target = gix::objs::TagRefIter::from_bytes(data)
                .target_id()
                .map_err(|error| Error::Protocol(error.to_string()))?;
            dependencies.push(target);
        }
        gix::object::Kind::Blob => {}
    }
    Ok(dependencies)
}

fn encode_pack(objects: Vec<PackObject>) -> Result<Vec<u8>, Error> {
    let object_count = u32::try_from(objects.len())
        .map_err(|_| Error::Protocol("pack contains too many objects".to_string()))?;
    let mut pack = Vec::new();
    pack.extend_from_slice(b"PACK");
    pack.extend_from_slice(&2u32.to_be_bytes());
    pack.extend_from_slice(&object_count.to_be_bytes());

    for object in objects {
        let kind = match object.kind {
            gix::object::Kind::Commit => 1,
            gix::object::Kind::Tree => 2,
            gix::object::Kind::Blob => 3,
            gix::object::Kind::Tag => 4,
        };
        encode_object_header(kind, object.data.len(), &mut pack);
        pack.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(&object.data, 6));
    }

    let checksum = Sha1::digest(&pack);
    pack.extend_from_slice(&checksum);
    Ok(pack)
}

fn encode_object_header(kind: u8, size: usize, output: &mut Vec<u8>) {
    let mut remaining = size >> 4;
    let mut first = (kind << 4) | (size as u8 & 0x0f);
    if remaining > 0 {
        first |= 0x80;
    }
    output.push(first);
    while remaining > 0 {
        let mut next = (remaining & 0x7f) as u8;
        remaining >>= 7;
        if remaining > 0 {
            next |= 0x80;
        }
        output.push(next);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wants_haves_and_sideband_capability() {
        let oid = "0123456789012345678901234567890123456789";
        let want = format!("want {oid}\0side-band-64k\n");
        let mut body = encode_pkt_line(want.as_bytes()).unwrap();
        body.extend_from_slice(b"0000");
        body.extend(encode_pkt_line(format!("have {oid}\n").as_bytes()).unwrap());
        body.extend_from_slice(b"0000");
        body.extend(encode_pkt_line(b"done\n").unwrap());

        let request = parse_request(&body).unwrap();

        assert_eq!(request.wants.len(), 1);
        assert_eq!(request.haves.len(), 1);
        assert!(request.done);
        assert!(request.sideband);
    }

    #[test]
    fn encodes_pack_header_and_checksum() {
        let pack = encode_pack(vec![PackObject {
            kind: gix::object::Kind::Blob,
            data: b"data".to_vec(),
        }])
        .unwrap();

        assert_eq!(&pack[..4], b"PACK");
        assert_eq!(u32::from_be_bytes(pack[4..8].try_into().unwrap()), 2);
        assert_eq!(u32::from_be_bytes(pack[8..12].try_into().unwrap()), 1);
        assert_eq!(
            Sha1::digest(&pack[..pack.len() - 20]).as_slice(),
            &pack[pack.len() - 20..]
        );
    }
}
