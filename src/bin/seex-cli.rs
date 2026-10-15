//! Console entry point. Original GUI remains src/main.rs.
use clap::{CommandFactory, Parser};
use seex::{
    cli::{AltiumCommands, Cli, Commands, KicadCommands, KicadVersion},
    lceda::LcedaClient,
};
use serde_json::{Value, json};
use std::{fs, path::Path};

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn print_json(value: Value) {
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}

fn main() {
    let cli = Cli::parse();
    if cli.prompt {
        println!(
            "SeEx by Ref42 — local CLI adaptation\nseex-cli scan-project board.epro2 -o inventory.json\nseex-cli search C2040\nseex-cli download-step C2040 --output staging\nseex-cli kicad export C2040 --full -o staging --library-name ASP200"
        );
        return;
    }
    let Some(command) = cli.command else {
        Cli::command().print_help().unwrap();
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    match runtime.block_on(run(command)) {
        Ok((value, failed)) => {
            print_json(value);
            if failed {
                std::process::exit(2);
            }
        }
        Err(e) => {
            eprintln!("{}", json!({"status":"error","error":e.to_string()}));
            std::process::exit(1);
        }
    }
}

fn atomic_json(path: &Path, v: &Value) -> AnyResult<()> {
    if path.exists() {
        return Err(format!("output exists: {}", path.display()).into());
    }
    if let Some(p) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(p)?;
    }
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    f.write_all(serde_json::to_string_pretty(v)?.as_bytes())?;
    Ok(())
}

async fn run(command: Commands) -> AnyResult<(Value, bool)> {
    let client = LcedaClient::new();
    let result = match command {
        Commands::ScanProject {
            input,
            output,
            ids_output,
        } => {
            if input.canonicalize()? == output.canonicalize().unwrap_or(output.clone()) {
                return Err("input and output must differ".into());
            }
            let v = seex::project_scan::scan_file(&input)?;
            if let Some(p) = &ids_output {
                if p.exists() || p == &output {
                    return Err("IDs output must be a new, different file".into());
                }
            }
            atomic_json(&output, &v)?;
            if let Some(p) = &ids_output {
                use std::{collections::BTreeSet, io::Write};
                let ids: BTreeSet<&str> = v["documents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|d| {
                        d["lcsc_ids"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .filter_map(Value::as_str)
                    })
                    .collect();
                if let Some(parent) = p.parent().filter(|p| !p.as_os_str().is_empty()) {
                    fs::create_dir_all(parent)?;
                }
                let mut f = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(p)?;
                for id in ids {
                    writeln!(f, "{id}")?;
                }
            }
            let components: usize = v["documents"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d["components"].as_array().unwrap().len())
                .sum();
            json!({"status":"ok","output":output,"ids_output":ids_output,"components":components})
        }
        Commands::ModelUuid {
            uuid,
            output,
            step_asset,
        } => {
            let id = uuid.split('|').next().unwrap_or("");
            if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("expected a 32-hex ONLINE model UUID; local project and KiCad instance IDs cannot be used".into());
            }
            let resolved = if step_asset {
                id.to_string()
            } else {
                let item = seex::lceda::SearchItem {
                    index: 1,
                    display_title: String::new(),
                    title: String::new(),
                    manufacturer: String::new(),
                    model_uuid: Some(id.to_string()),
                    raw: Value::Null,
                };
                client.get_model_uuid(&item).await?
            };
            let bytes = client.download_step_bytes(&resolved).await?;
            if !bytes.windows(13).any(|w| w == b"ISO-10303-21;") {
                return Err("response is not an ISO STEP document".into());
            }
            if let Some(p) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(p)?;
            }
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            f.write_all(&bytes)?;
            json!({"status":"ok","model_reference_uuid":id,"step_asset_uuid":resolved,"output":output,"bytes":bytes.len()})
        }
        Commands::Search { keyword, limit } => {
            let mut items = client.search_components(&keyword).await?;
            if items.is_empty()
                && seex::extract::extract_lcsc_ids(&keyword) == vec![keyword.to_uppercase()]
            {
                items.push(client.select_item(&keyword, 1).await?);
            }
            json!({"status":"ok","results":items.iter().take(limit).map(|i|json!({"index":i.index,"name":i.display_name(),"manufacturer":i.manufacturer,"lcsc_id":i.lcsc_id(),"symbol_uuid":i.symbol_uuid(),"footprint_uuid":i.footprint_uuid(),"model_uuid":i.model_uuid})).collect::<Vec<_>>()})
        }
        Commands::DownloadStep {
            keyword,
            index,
            output,
            force,
        } => {
            let item = client.select_item(&keyword, index).await?;
            let path =
                seex::altium::workflow::download_step(&client, &item, &output, force).await?;
            let bytes = fs::read(&path)?;
            if !bytes.windows(13).any(|w| w == b"ISO-10303-21;") {
                return Err(format!("downloaded file is not STEP: {}", path.display()).into());
            }
            json!({"status":"ok","output":path,"bytes":bytes.len()})
        }
        Commands::DownloadObj {
            keyword,
            index,
            output,
            force,
        } => {
            let item = client.select_item(&keyword, index).await?;
            let (obj, mtl) =
                seex::altium::workflow::download_obj(&client, &item, &output, force).await?;
            json!({"status":"ok","obj":obj,"mtl":mtl})
        }
        Commands::Source {
            keyword,
            index,
            output,
            force,
        } => {
            let item = client.select_item(&keyword, index).await?;
            json!({"status":"ok","files":seex::altium::workflow::export_easyeda_sources(&client,&item,&output,force).await?})
        }
        Commands::Bundle {
            keyword,
            index,
            output,
            force,
        } => {
            let item = client.select_item(&keyword, index).await?;
            json!({"status":"ok","files":seex::altium::workflow::export_bundle(&client,&item,&output,force).await?})
        }
        Commands::Kicad { command } => {
            let (ids, library, export, english_metadata, parallel, continue_on_error) =
                match command {
                    KicadCommands::Export {
                        component_id,
                        symbol,
                        footprint,
                        model_3d,
                        full,
                        output,
                        library_name,
                        symbol_lib,
                        footprint_lib,
                        model_lib,
                        force,
                        english_metadata,
                    } => (
                        vec![component_id],
                        seex::kicad::KicadLibraryOptions {
                            output,
                            library_name,
                            symbol_lib,
                            footprint_lib,
                            model_lib,
                        },
                        seex::kicad::KicadExportOptions {
                            symbol,
                            footprint,
                            model_3d,
                            full,
                            force,
                            kicad_version: KicadVersion::V6,
                        },
                        english_metadata,
                        1,
                        false,
                    ),
                    KicadCommands::Batch {
                        input,
                        symbol,
                        footprint,
                        model_3d,
                        full,
                        output,
                        library_name,
                        symbol_lib,
                        footprint_lib,
                        model_lib,
                        parallel,
                        continue_on_error,
                        force,
                        english_metadata,
                    } => (
                        seex::extract::extract_lcsc_ids(&fs::read_to_string(input)?),
                        seex::kicad::KicadLibraryOptions {
                            output,
                            library_name,
                            symbol_lib,
                            footprint_lib,
                            model_lib,
                        },
                        seex::kicad::KicadExportOptions {
                            symbol,
                            footprint,
                            model_3d,
                            full,
                            force,
                            kicad_version: KicadVersion::V6,
                        },
                        english_metadata,
                        parallel,
                        continue_on_error,
                    ),
                };
            if parallel == 0 || parallel > 16 {
                return Err("parallel must be 1..16".into());
            }
            // Keep generated libraries in staging; project updates belong to KiCad/Konnect.
            for p in [&library.output]
                .into_iter()
                .chain(library.symbol_lib.iter())
                .chain(library.footprint_lib.iter())
                .chain(library.model_lib.iter())
            {
                reject_project_target(p)?;
            }
            let s = seex::kicad::export_kicad_batch(seex::kicad::KicadBatchOptions {
                ids,
                library,
                export,
                english_metadata,
                parallel,
                continue_on_error,
                progress: None,
            })
            .await?;
            let failed = s.failed > 0 || s.models.missing > 0;
            return Ok((
                json!({"status":if failed{"partial"}else{"ok"},"total":s.total,"success":s.success,"skipped":s.skipped,"failed":s.failed,"failed_ids":s.failed_ids,"failure_details":s.failure_details,"files":s.generated_files,"models":{"written":s.models.written,"skipped":s.models.skipped,"missing":s.models.missing}}),
                failed,
            ));
        }
        Commands::Altium { command } => {
            let (
                input,
                output,
                schlib,
                pcblib,
                full,
                merge,
                append,
                library_name,
                parallel,
                continue_on_error,
                english_metadata,
                force,
                temp,
            ) = match command {
                AltiumCommands::Export {
                    component,
                    index,
                    output,
                    schlib,
                    pcblib,
                    full,
                    english_metadata,
                    force,
                } => {
                    // Single-keyword Altium exports retain the upstream search-index workflow.
                    let item = client.select_item(&component, index).await?;
                    let mut paths = vec![];
                    if schlib || full {
                        paths.push(
                            seex::altium::workflow::export_schlib_with_options(
                                &client,
                                &item,
                                &output,
                                force,
                                english_metadata,
                                0x68380B,
                                0xE0FFFF,
                            )
                            .await?,
                        );
                    }
                    if pcblib || full {
                        paths.push(
                            seex::altium::workflow::export_pcblib(&client, &item, &output, force)
                                .await?,
                        );
                    }
                    if paths.is_empty() {
                        return Err("select --schlib, --pcblib or --full".into());
                    }
                    return Ok((json!({"status":"ok","files":paths}), false));
                }
                AltiumCommands::Batch {
                    input,
                    output,
                    schlib,
                    pcblib,
                    full,
                    merge,
                    append,
                    library_name,
                    parallel,
                    continue_on_error,
                    english_metadata,
                    force,
                } => (
                    input,
                    output,
                    schlib,
                    pcblib,
                    full,
                    merge,
                    append,
                    library_name,
                    parallel,
                    continue_on_error,
                    english_metadata,
                    force,
                    false,
                ),
            };
            let _ = temp;
            let s = seex::batch::export_batch(
                &client,
                seex::batch::BatchOptions {
                    input,
                    output,
                    schlib,
                    pcblib,
                    full,
                    merge,
                    append,
                    library_name,
                    parallel,
                    continue_on_error,
                    english_metadata,
                    force,
                    schematic_stroke_color: [128, 0, 0],
                    schematic_fill_color: [255, 255, 176],
                    progress: None,
                },
            )
            .await?;
            return Ok((
                json!({"status":if s.failed>0{"partial"}else{"ok"},"total":s.total,"success":s.success,"skipped":s.skipped,"failed":s.failed,"failed_ids":s.failed_ids,"files":s.generated_files}),
                s.failed > 0,
            ));
        }
    };
    Ok((result, false))
}

fn reject_project_target(path: &Path) -> AnyResult<()> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut existing = absolute.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or("invalid output path")?;
    }
    let canonical = existing.canonicalize()?;
    for ancestor in canonical.ancestors() {
        if fs::read_dir(ancestor)?.filter_map(Result::ok).any(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x == "kicad_pro" || x == "kicad_pcb")
        }) {
            return Err("export into a separate staging folder, not a KiCad project/library; integrate through Konnect".into());
        }
    }
    Ok(())
}
