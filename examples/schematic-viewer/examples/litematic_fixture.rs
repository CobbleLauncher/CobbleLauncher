// SPDX-License-Identifier: MIT
//! Original synthetic Version 4–7 fixture (default 6) for actual Component smoke tests.
use fastnbt::{LongArray, Value};
use std::{collections::HashMap, io::Write};
fn vector(x: i32, y: i32, z: i32) -> Value {
    Value::Compound(HashMap::from([
        ("x".into(), Value::Int(x)),
        ("y".into(), Value::Int(y)),
        ("z".into(), Value::Int(z)),
    ]))
}
fn state(name: &str) -> Value {
    Value::Compound(HashMap::from([("Name".into(), Value::String(name.into()))]))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("Supply a new output .litematic path and optional version 4–7")?;
    let version = args
        .next()
        .map(|value| {
            value
                .to_str()
                .ok_or("Version must be UTF-8")?
                .parse::<i32>()
                .map_err(|_| "Version must be an integer 4–7")
        })
        .transpose()?
        .unwrap_or(6);
    if !matches!(version, 4..=7) || args.next().is_some() {
        return Err("Usage: litematic_fixture <new-output.litematic> [4|5|6|7]".into());
    }
    let palette = std::iter::once(state("minecraft:air"))
        .chain((1..26).map(|id| state(&format!("fixture:block_{id}"))))
        .collect();
    // Negative X size: primary corner x12, actual minimum x0. Thirteen cells;
    // ID25 is stored across the 64-bit boundary at cell12; other cells are air.
    let region = Value::Compound(HashMap::from([
        ("Position".into(), vector(12, 0, 0)),
        ("Size".into(), vector(-13, 1, 1)),
        ("BlockStatePalette".into(), Value::List(palette)),
        (
            "BlockStates".into(),
            Value::LongArray(LongArray::new(vec![i64::MIN + (1 << 60), 1])),
        ),
        ("Entities".into(), Value::List(vec![])),
        ("TileEntities".into(), Value::List(vec![])),
    ]));
    let second = Value::Compound(HashMap::from([
        ("Position".into(), vector(15, 0, 0)),
        ("Size".into(), vector(1, 1, 1)),
        (
            "BlockStatePalette".into(),
            Value::List(vec![state("minecraft:air"), state("minecraft:stone")]),
        ),
        (
            "BlockStates".into(),
            Value::LongArray(LongArray::new(vec![1])),
        ),
    ]));
    let root: HashMap<String, Value> = HashMap::from([
        ("Version".into(), Value::Int(version)),
        (
            "SubVersion".into(),
            Value::Int(if version <= 5 { 0 } else { 1 }),
        ),
        (
            "MinecraftDataVersion".into(),
            Value::Int(match version {
                4 => 1343,
                5 => 2230,
                6 => 3120,
                _ => 3837,
            }),
        ),
        (
            "Metadata".into(),
            Value::Compound(HashMap::from([
                (
                    "Name".into(),
                    Value::String("Original boundary fixture".into()),
                ),
                ("Author".into(), Value::String("CobbleLauncher".into())),
                ("EnclosingSize".into(), vector(16, 1, 1)),
                ("RegionCount".into(), Value::Int(2)),
                ("TotalVolume".into(), Value::Int(14)),
                ("TotalBlocks".into(), Value::Int(2)),
            ])),
        ),
        (
            "Regions".into(),
            Value::Compound(HashMap::from([
                ("boundary".into(), region),
                ("separate".into(), second),
            ])),
        ),
    ]);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut gzip = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    gzip.write_all(&fastnbt::to_bytes(&root)?)?;
    gzip.finish()?;
    Ok(())
}
