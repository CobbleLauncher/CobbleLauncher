// SPDX-License-Identifier: MIT
use fastnbt::Value;
use std::{collections::HashMap, error::Error};
fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Supply a new .schem output path")?;
    let palette = Value::Compound(HashMap::from([
        ("minecraft:air".into(), Value::Int(0)),
        ("minecraft:stone".into(), Value::Int(1)),
        ("minecraft:oak_planks".into(), Value::Int(2)),
    ]));
    let mut blocks = Vec::new();
    for y in 0..8 {
        for z in 0..16 {
            for x in 0..16 {
                blocks.push(if y == 0 {
                    1
                } else if y < 6 && (x == 0 || x == 15 || z == 0 || z == 15) {
                    2
                } else {
                    0
                });
            }
        }
    }
    let root = HashMap::from([
        ("Version", Value::Int(2)),
        ("Width", Value::Short(16)),
        ("Height", Value::Short(8)),
        ("Length", Value::Short(16)),
        ("Palette", palette),
        (
            "BlockData",
            Value::ByteArray(fastnbt::ByteArray::new(blocks)),
        ),
    ]);
    let bytes = fastnbt::to_bytes(&root)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&bytes)?;
    Ok(())
}
use std::io::Write;
