use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::exit;
use std::{fs::File, io::BufReader};

use eurochef_edb::binrw::BinReaderExt;
use eurochef_edb::{edb::EdbFile, versions::Platform, Hashcode};
use eurochef_shared::filesystem::path::DissectedFilelistPath;
use eurochef_shared::maps::{format_hashcode, DefinitionDataType};
use eurochef_shared::robots_runtime::inventory::{
    RobotsInventoryDefinition, ROBOTS_INVENTORY_FILE_UID, ROBOTS_INVENTORY_SPREADSHEET_UID,
};
use eurochef_shared::robots_runtime::mission::{
    RobotsMissionDefinition, ROBOTS_MISSIONS_FILE_UID, ROBOTS_MISSIONS_SPREADSHEET_UID,
};
use eurochef_shared::robots_runtime::shop::{
    read_robots_shop_database, ROBOTS_SHOP_FILE_UID, ROBOTS_SHOP_SPREADSHEET_UID,
};
use eurochef_shared::spreadsheets::{SpreadsheetDefinitions, UXGeoSpreadsheet};

pub fn execute_command(filename: String, output_folder: Option<String>) -> anyhow::Result<()> {
    let output_folder = output_folder.unwrap_or(format!(
        "./spreadsheets/{}/",
        Path::new(&filename).file_name().unwrap().to_string_lossy()
    ));
    let output_folder = Path::new(&output_folder);
    std::fs::create_dir_all(output_folder)?;

    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let mut edb = EdbFile::new(Box::new(reader), Platform::Pc)?;

    if edb.header.hashcode == ROBOTS_SHOP_FILE_UID {
        let database = read_robots_shop_database(&mut edb)?;
        let mut groups =
            File::create(output_folder.join(format!("{ROBOTS_SHOP_SPREADSHEET_UID:08x}_0.csv")))?;
        writeln!(groups, "shop_uid,slot,item_uid,price")?;
        for group in &database.groups {
            for (slot, item) in group.slots.iter().enumerate() {
                writeln!(
                    groups,
                    "0x{:08x},{slot},0x{:08x},{}",
                    group.shop_uid, item.item_uid, item.price
                )?;
            }
        }

        let mut items =
            File::create(output_folder.join(format!("{ROBOTS_SHOP_SPREADSHEET_UID:08x}_1.csv")))?;
        writeln!(items, "uid,word_04,display_resource_uid,word_0c,word_10,name_text_uid,description_text_uid,quantity_word_1c,purchase_quantity,word_20")?;
        for item in &database.items {
            writeln!(
                items,
                "0x{:08x},0x{:08x},0x{:08x},0x{:08x},0x{:08x},0x{:08x},0x{:08x},0x{:08x},{},0x{:08x}",
                item.uid,
                item.words_04_to_20[0],
                item.display_resource_uid(),
                item.words_04_to_20[2],
                item.words_04_to_20[3],
                item.name_text_uid(),
                item.description_text_uid(),
                item.words_04_to_20[6],
                item.purchase_quantity(),
                item.words_04_to_20[7]
            )?;
        }
        info!(
            groups = database.groups.len(),
            items = database.items.len(),
            "Successfully extracted Robots shop spreadsheet"
        );
        return Ok(());
    }

    let (spreadsheet_definitions, hashcodes) =
        if let Some(dissected_path) = DissectedFilelistPath::dissect(&filename) {
            let exe_path = std::env::current_exe().unwrap();
            let exe_dir = exe_path.parent().unwrap();
            let v = std::fs::read_to_string(
                exe_dir.join(format!("./assets/spreadsheets_{}.yml", dissected_path.game)),
            )
            .unwrap_or_default();

            let spreadsheet_definitions: SpreadsheetDefinitions = match serde_yaml::from_str(&v) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to parse spreadsheet definitions: {e}");
                    Default::default()
                }
            };

            (
                spreadsheet_definitions,
                eurochef_shared::filesystem::load_hashcodes(&dissected_path, true),
            )
        } else {
            error!("Given path is not a valid EngineX-compatible path");
            (Default::default(), Default::default())
        };

    let spreadsheet_definition = spreadsheet_definitions
        .get(&edb.header.hashcode)
        .cloned()
        .unwrap_or_default();

    for (file_hashcode, sfile) in &spreadsheet_definitions {
        for (hashcode, spreadsheet) in &sfile.0 {
            for sheet in spreadsheet.0.values() {
                if sheet.columns.is_empty() {
                    continue;
                }

                let total_column_size: usize = sheet.columns.iter().map(|v| v.dtype.size()).sum();
                if total_column_size != sheet.row_size {
                    error!("Spreadsheet {hashcode:08x} (file {file_hashcode:08x}) has an invalid row size (total row size {}, row_size {})", total_column_size, sheet.row_size);
                    exit(-1);
                }
            }
        }
    }

    let spreadsheets = UXGeoSpreadsheet::read_all(&mut edb)?;
    if spreadsheets.is_empty() {
        println!("No spreadsheets found in file");
        return Ok(());
    }

    for (hashcode, spreadsheet) in &spreadsheets {
        info!(
            "Extracting spreadsheet {hashcode:08x} ({} sheets)",
            match &spreadsheet {
                UXGeoSpreadsheet::Data(v) => v.len(),
                UXGeoSpreadsheet::Text(v) => v.len(),
            }
        );

        match spreadsheet {
            UXGeoSpreadsheet::Data(data) => {
                for (sheet_num, sheet) in data.iter().enumerate() {
                    edb.seek(SeekFrom::Start(sheet.address as u64))?;
                    if edb.header.hashcode == ROBOTS_MISSIONS_FILE_UID
                        && *hashcode == ROBOTS_MISSIONS_SPREADSHEET_UID
                    {
                        let mut output = File::create(
                            output_folder.join(format!("{hashcode:08x}_{sheet_num}.csv")),
                        )?;
                        writeln!(
                            output,
                            "mission_uid,objective_uid,hud_item_uid,mission_text_uid"
                        )?;
                        for _ in 0..sheet.row_count {
                            let words = [
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                            ];
                            if let Some(row) = RobotsMissionDefinition::from_native_words(words) {
                                writeln!(
                                    output,
                                    "0x{:08x},0x{:08x},0x{:08x},0x{:08x}",
                                    row.mission_uid,
                                    row.objective_uid,
                                    row.hud_item_uid,
                                    row.mission_text_uid
                                )?;
                            }
                        }
                        continue;
                    }
                    if edb.header.hashcode == ROBOTS_INVENTORY_FILE_UID
                        && *hashcode == ROBOTS_INVENTORY_SPREADSHEET_UID
                    {
                        let mut output = File::create(
                            output_folder.join(format!("{hashcode:08x}_{sheet_num}.csv")),
                        )?;
                        writeln!(
                            output,
                            "uid,word_04,word_08,word_0c,word_10,word_14,target_word_18,target"
                        )?;
                        for _ in 0..sheet.row_count {
                            let words = [
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                                edb.read_type(edb.endian)?,
                            ];
                            if let Some(row) = RobotsInventoryDefinition::from_native_words(words) {
                                writeln!(
                                    output,
                                    "0x{:08x},0x{:08x},0x{:08x},0x{:08x},0x{:08x},0x{:08x},0x{:08x},{}",
                                    row.uid,
                                    row.words_04_to_14[0],
                                    row.words_04_to_14[1],
                                    row.words_04_to_14[2],
                                    row.words_04_to_14[3],
                                    row.words_04_to_14[4],
                                    row.target_word_18,
                                    row.target(0, 0)
                                )?;
                            }
                        }
                        continue;
                    }
                    let sheet_definition = match spreadsheet_definition.0.get(hashcode) {
                        None => {
                            error!("Missing spreadsheet definition for file {:08x} spreadsheet {hashcode:08x} sheet #{sheet_num} (address 0x{:x})", edb.header.hashcode, sheet.address);
                            continue;
                        }
                        Some(s) => match s.0.get(&sheet_num) {
                            None => {
                                error!("Missing sheet definition for file {:08x} spreadsheet {hashcode:08x} sheet #{sheet_num} (address 0x{:x})", edb.header.hashcode, sheet.address);
                                continue;
                            }
                            Some(s) => s,
                        },
                    };
                    let mut output = File::create(
                        output_folder.join(format!("{hashcode:08x}_{sheet_num}.csv")),
                    )?;

                    if sheet_definition.columns.is_empty() {
                        warn!("Missing column definitions for file {:08x} spreadsheet {hashcode:08x} sheet #{sheet_num} (address 0x{:x})", edb.header.hashcode, sheet.address);
                        writeln!(output, "data")?;
                        let mut row_data = vec![0u8; sheet_definition.row_size];
                        for _ in 0..sheet.row_count {
                            edb.read_exact(&mut row_data)?;
                            writeln!(output, "{}", hex::encode(&row_data))?;
                        }
                    } else {
                        let header = sheet_definition
                            .columns
                            .iter()
                            .enumerate()
                            .map(|(i, c)| c.name.clone().unwrap_or(format!("row_{i}")))
                            .collect::<Vec<String>>()
                            .join(",");
                        writeln!(output, "{}", header)?;

                        for _ in 0..sheet.row_count {
                            let mut row = vec![];

                            for c in &sheet_definition.columns {
                                match c.dtype {
                                    DefinitionDataType::Unknown32 => {
                                        let v: u32 = edb.read_type(edb.endian)?;
                                        row.push(format!("0x{v:x}"));
                                    }
                                    DefinitionDataType::U32 => {
                                        let v: u32 = edb.read_type(edb.endian)?;
                                        row.push(v.to_string());
                                    }
                                    DefinitionDataType::Float => {
                                        let v: f32 = edb.read_type(edb.endian)?;
                                        row.push(v.to_string());
                                    }
                                    DefinitionDataType::Hashcode => {
                                        let v: Hashcode = edb.read_type(edb.endian)?;
                                        row.push(format_hashcode(&hashcodes, v));
                                    }
                                    // ROBOTS_PATCH_0027_PICKUP_CLI_EXPORT
                                    DefinitionDataType::Pickup => {
                                        let v: u32 = edb.read_type(edb.endian)?;
                                        let pickup_hashcode = if (v & 0xFF000000) == 0x47000000 {
                                            v
                                        } else {
                                            0x47000000 | v
                                        };
                                        row.push(format!(
                                            "{} ({})",
                                            format_hashcode(&hashcodes, pickup_hashcode),
                                            v
                                        ));
                                    }
                                    DefinitionDataType::ScriptCreateFlags => {
                                        let v: u32 = edb.read_type(edb.endian)?;
                                        row.push(c.dtype.to_string(&hashcodes, v));
                                    }
                                }
                            }

                            writeln!(output, "{}", row.join(","))?;
                        }
                    }
                }
            }

            UXGeoSpreadsheet::Text(text) => {
                for (i, s) in text.iter().enumerate() {
                    let mut output =
                        File::create(output_folder.join(format!("{hashcode:08x}_{i}.csv")))?;
                    writeln!(output, "# Section {:08x}", s.hashcode)?;
                    spreadsheet.export_text_to_csv(&mut output, s.hashcode)?;
                }
            }
        }
    }

    info!("Successfully extracted spreadsheets!");

    Ok(())
}
