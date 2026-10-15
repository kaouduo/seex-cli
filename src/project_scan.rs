//! Read-only EasyEDA Pro record extraction; local IDs are never online UUIDs.
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

type ScanResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn scan_text(text: &str, member: &str) -> ScanResult<Value> {
    let mut doc_type = String::new();
    let mut doc_id = String::new();
    let mut devices = BTreeMap::<String, Value>::new();
    let mut components = BTreeMap::<(String, String), Value>::new();
    let mut attrs = BTreeMap::<(String, String), BTreeMap<String, Value>>::new();
    let mut attr_records = BTreeMap::<(String, String), Value>::new();
    let mut tombstones = 0usize;
    let mut counts = BTreeMap::<String, usize>::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (head, body) = line
            .split_once("||")
            .ok_or_else(|| format!("{member}:{}: missing record separator", i + 1))?;
        let h: Value = serde_json::from_str(head)?;
        let record_key = (doc_id.clone(), h["id"].as_str().unwrap_or("").to_string());
        if body == "|" || body.is_empty() {
            tombstones += 1;
            match (doc_type.as_str(), h["type"].as_str().unwrap_or("")) {
                ("SCH_PAGE", "COMPONENT") => {
                    components.remove(&record_key);
                }
                ("SCH_PAGE", "ATTR") => {
                    attr_records.remove(&record_key);
                }
                ("DEVICE", "META") => {
                    devices.remove(&doc_id);
                }
                (_, "DOCHEAD") => return Err("deleted document header is unsupported".into()),
                _ => (),
            }
            continue;
        }
        // Remove exactly the record terminator, preserving pipes within JSON strings.
        let b: Value = serde_json::from_str(body.strip_suffix('|').unwrap_or(body))?;
        match h["type"].as_str().unwrap_or("") {
            "DOCHEAD" => {
                doc_type = b["docType"].as_str().unwrap_or("").into();
                doc_id = b["uuid"].as_str().unwrap_or("").into();
                *counts.entry(doc_type.clone()).or_default() += 1;
            }
            "META" if doc_type == "DEVICE" => {
                devices.insert(doc_id.clone(), b);
            }
            "COMPONENT" if doc_type == "SCH_PAGE" => {
                components.insert((doc_id.clone(), h["id"].as_str().unwrap_or("").into()), b);
            }
            "ATTR" if doc_type == "SCH_PAGE" => {
                attr_records.insert(record_key, b);
            }
            _ => (),
        }
    }
    if counts.is_empty() {
        return Err("no EasyEDA Pro documents found".into());
    }
    for ((sheet, _), b) in attr_records {
        let key = (sheet, b["parentId"].as_str().unwrap_or("").into());
        attrs
            .entry(key)
            .or_default()
            .insert(b["key"].as_str().unwrap_or("").into(), b["value"].clone());
    }
    let mut rows = vec![];
    let mut ids = BTreeSet::new();
    for ((sheet, instance), c) in components {
        let a = attrs
            .get(&(sheet.clone(), instance.clone()))
            .cloned()
            .unwrap_or_default();
        let reference = a
            .get("Designator")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        // Power symbols and graphical components do not have physical designators.
        if reference.is_empty() || reference.contains('?') {
            continue;
        }
        let local = a
            .get("Device")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let d = devices.get(&local).cloned().unwrap_or(Value::Null);
        let mut merged = d["attributes"].as_object().cloned().unwrap_or_default();
        merged.extend(a.into_iter());
        let supplier = merged
            .get("Supplier Part")
            .and_then(Value::as_str)
            .unwrap_or("");
        let ccodes = crate::extract::extract_lcsc_ids(supplier);
        let ccode = ccodes.first().cloned();
        if let Some(id) = &ccode {
            ids.insert(id.clone());
        }
        rows.push(json!({"reference":reference,"sheet_local_id":sheet,"instance_local_id":instance,
            "device_local_id":local,"device_source":d["source"],"title":d["title"],
            "lcsc_id":ccode,"symbol_local_id":merged.get("Symbol"),
            "footprint_local_id":merged.get("Footprint"),"model_reference":merged.get("3D Model"),
            "model_transform_raw":merged.get("3D Model Transform"),"attributes":merged,
            "position_raw":c,"lookup_status":if ccode.is_some(){"lcsc_id_available"}else{"needs_resolution"}}));
    }
    Ok(
        json!({"member":member,"document_counts":counts,"tombstone_count":tombstones,"components":rows,"lcsc_ids":ids,
        "note":"Local instance/device/footprint IDs are not online UUIDs. Preserve model transforms; do not infer placement from UUID alone."}),
    )
}

pub fn scan_file(input: &Path) -> ScanResult<Value> {
    let mut docs = vec![];
    if input.extension().and_then(|x| x.to_str()) == Some("epru") {
        docs.push(scan_text(
            &fs::read_to_string(input)?,
            &input.display().to_string(),
        )?);
    } else {
        let mut zip = zip::ZipArchive::new(fs::File::open(input)?)?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            if !f.name().ends_with(".epru") {
                continue;
            }
            if f.size() > 128 * 1024 * 1024 {
                return Err("project record member exceeds 128 MiB".into());
            }
            let name = f.name().to_string();
            let mut text = String::new();
            f.read_to_string(&mut text)?;
            docs.push(scan_text(&text, &name)?);
        }
    }
    if docs.is_empty() {
        return Err("archive contains no .epru record stream".into());
    }
    Ok(json!({"schema_version":1,"source":input,"documents":docs}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_forward_device_metadata_and_keeps_model_transform() {
        let text = concat!(
            "{\"type\":\"DOCHEAD\"}||{\"docType\":\"SCH_PAGE\",\"uuid\":\"page\"}|\n",
            "{\"type\":\"COMPONENT\",\"id\":\"i\"}||{}|\n",
            "{\"type\":\"ATTR\",\"id\":\"a1\"}||{\"parentId\":\"i\",\"key\":\"Designator\",\"value\":\"U1\"}|\n",
            "{\"type\":\"ATTR\",\"id\":\"a2\"}||{\"parentId\":\"i\",\"key\":\"Device\",\"value\":\"local16\"}|\n",
            "{\"type\":\"DOCHEAD\"}||{\"docType\":\"DEVICE\",\"uuid\":\"local16\"}|\n",
            "{\"type\":\"META\"}||{\"attributes\":{\"Supplier Part\":\"C2040\",\"3D Model\":\"model|owner\",\"3D Model Transform\":\"1,2,3\"}}|\n"
        );
        let r = scan_text(text, "test").unwrap();
        assert_eq!(r["components"][0]["lcsc_id"], "C2040");
        assert_eq!(r["components"][0]["device_local_id"], "local16");
        assert_eq!(r["components"][0]["model_reference"], "model|owner");
        assert_eq!(r["components"][0]["model_transform_raw"], "1,2,3");
    }
    #[test]
    fn malformed_record_is_not_silently_skipped() {
        assert!(scan_text("bad record", "test").is_err());
        assert!(scan_text("", "test").is_err());
    }

    #[test]
    fn tombstone_removes_component() {
        let text = concat!(
            "{\"type\":\"DOCHEAD\"}||{\"docType\":\"SCH_PAGE\",\"uuid\":\"page\"}|\n",
            "{\"type\":\"COMPONENT\",\"id\":\"i\"}||{}|\n",
            "{\"type\":\"ATTR\",\"id\":\"a1\"}||{\"parentId\":\"i\",\"key\":\"Designator\",\"value\":\"R1\"}|\n",
            "{\"type\":\"COMPONENT\",\"id\":\"i\"}|||\n"
        );
        let r = scan_text(text, "test").unwrap();
        assert_eq!(r["tombstone_count"], 1);
        assert_eq!(r["components"].as_array().unwrap().len(), 0);
    }
}
