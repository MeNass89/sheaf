//! Read encrypted COS objects without authenticating a password security handler.
//! Compressed object streams are refused explicitly until their recipient-key reader is wired.
use super::{crypto, unsupported, Result, Scope};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat, xref::XrefEntry};
use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use pdfcraft_cos as cos;

fn length(bytes: &[u8], doc: &Document, reference: cos::ObjRef, depth: usize) -> Option<i64> {
    if depth > 8 { return None; }
    let XrefEntry::Normal { offset, generation } = doc.reference_table.get(reference.num)? else { return None; };
    if *generation != reference.generation { return None; }
    let (id, object) = cos::parse_indirect(bytes, *offset as usize, &|r| length(bytes, doc, r, depth + 1)).ok()?;
    (id == reference).then_some(object.as_int()).flatten()
}

fn dictionary(dict: &cos::Dict) -> Result<Dictionary> {
    let mut out = Dictionary::new();
    for (name, value) in dict.iter() { out.set(name.clone(), convert(value)?); }
    Ok(out)
}

fn convert(object: &cos::Object) -> Result<Object> {
    Ok(match object {
        cos::Object::Null => Object::Null,
        cos::Object::Bool(v) => Object::Boolean(*v),
        cos::Object::Int(v) => Object::Integer(*v),
        cos::Object::Real(v) => Object::Real(*v as f32),
        cos::Object::Name(v) => Object::Name(v.clone()),
        cos::Object::String(v) => Object::String(v.bytes.clone(), if v.hex { StringFormat::Hexadecimal } else { StringFormat::Literal }),
        cos::Object::Ref(v) => Object::Reference((v.num, v.generation)),
        cos::Object::Array(v) => Object::Array(v.iter().map(convert).collect::<Result<_>>()?),
        cos::Object::Dict(v) => Object::Dictionary(dictionary(v)?),
        cos::Object::Stream(v) => Object::Stream(Stream::new(dictionary(&v.dict)?, v.raw.to_vec())),
    })
}

pub(super) fn read(bytes: &[u8], mut doc: Document) -> Result<Document> {
    if doc.reference_table.entries.values().any(|e| matches!(e, XrefEntry::Compressed { .. })) {
        return Err(unsupported("Certificate-encrypted object streams are not supported yet. This offline workflow can reopen PDFs encrypted by Sheaf."));
    }
    let mut objects = std::collections::BTreeMap::new();
    for (&number, entry) in &doc.reference_table.entries {
        let XrefEntry::Normal { offset, generation } = entry else { continue; };
        let (id, object) = cos::parse_indirect(bytes, *offset as usize, &|r| length(bytes, &doc, r, 0)).map_err(crypto)?;
        if id.num != number || id.generation != *generation { return Err(unsupported("A certificate-encrypted PDF has an inconsistent object reference")); }
        objects.insert((number, *generation), convert(&object)?);
    }
    doc.objects = objects;
    Ok(doc)
}

fn named_type(dict: &Dictionary, name: &[u8]) -> bool { dict.get(b"Type").and_then(Object::as_name).ok() == Some(name) }

fn walk_dict(dict: &mut Dictionary, key: &[u8], scope: Scope, decrypt: bool) -> Result<()> {
    let signature = named_type(dict, b"Sig");
    for (name, object) in dict.iter_mut() {
        if signature && name == b"Contents" { continue; }
        walk(object, key, scope, decrypt)?;
    }
    Ok(())
}

fn walk(object: &mut Object, key: &[u8], scope: Scope, decrypt: bool) -> Result<()> {
    match object {
        Object::String(bytes, _) if scope != Scope::Attachments => {
            *bytes = if decrypt { Aes256CryptFilter.decrypt(key, bytes) } else { Aes256CryptFilter.encrypt(key, bytes) }.map_err(crypto)?;
        }
        Object::Array(array) => { for object in array { walk(object, key, scope, decrypt)?; } }
        Object::Dictionary(dict) => walk_dict(dict, key, scope, decrypt)?,
        Object::Stream(stream) => {
            if named_type(&stream.dict, b"XRef") || (scope == Scope::ExceptMetadata && named_type(&stream.dict, b"Metadata")) { return Ok(()); }
            // Respect explicit Identity, and refuse a filter we cannot preserve safely.
            if let Ok(filters) = stream.dict.get(b"Filter") {
                let crypt = filters.as_name().ok() == Some(b"Crypt".as_slice()) || filters.as_array().is_ok_and(|a| a.iter().any(|o| o.as_name().ok() == Some(b"Crypt".as_slice())));
                if crypt { return Err(unsupported("Explicit per-stream Crypt filters are not supported by this certificate workflow")); }
            }
            let encrypted = match scope {
                Scope::All => true,
                Scope::ExceptMetadata => !named_type(&stream.dict, b"Metadata"),
                Scope::Attachments => named_type(&stream.dict, b"EmbeddedFile"),
            };
            walk_dict(&mut stream.dict, key, scope, decrypt)?;
            if encrypted {
                stream.content = if decrypt { Aes256CryptFilter.decrypt(key, &stream.content) } else { Aes256CryptFilter.encrypt(key, &stream.content) }.map_err(crypto)?;
                stream.dict.set("Length", stream.content.len() as i64);
            }
        }
        _ => {},
    }
    Ok(())
}

pub(super) fn transform(doc: &mut Document, encryption: Option<ObjectId>, key: &[u8], scope: Scope, decrypt: bool) -> Result<()> {
    for (id, object) in &mut doc.objects {
        if Some(*id) != encryption { walk(object, key, scope, decrypt)?; }
    }
    Ok(())
}
