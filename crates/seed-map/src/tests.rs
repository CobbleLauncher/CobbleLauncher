// SPDX-License-Identifier: MIT

use super::*;
fn map(seed: &str) -> MapRequest {
    MapRequest {
        seed: seed.into(),
        version: "1.21.1".into(),
        dimension: "overworld".into(),
        preset: "default".into(),
        y: 64,
        x: -512,
        z: -512,
        width: 8,
        height: 8,
        step: 64,
    }
}
fn query(id: &str) -> SearchRequest {
    SearchRequest {
        seed: "0".into(),
        version: "1.21.1".into(),
        dimension: "overworld".into(),
        preset: "default".into(),
        y: 64,
        x: 0,
        z: 0,
        radius: 2048,
        target: SearchTarget {
            kind: "structure".into(),
            id: id.into(),
        },
        limit: 4,
    }
}
#[test]
fn pinned_upstream_vectors() {
    // Original regression fixtures collected from this pinned upstream;
    // these guard integration/precision, not real-game terrain acceptance.
    const NORMAL: [i32; 64] = [
        0, 0, 24, 0, 24, 24, 24, 24, 0, 0, 0, 0, 0, 24, 24, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0,
        0, 4, 4, 4, 16, 16, 7, 16, 16, 4, 4, 27, 27, 4, 27, 7, 7, 27, 27, 27, 27, 4, 27, 27, 27,
        27, 27, 4, 4, 27, 27, 27, 27, 27, 7, 4, 4,
    ];
    const CAVES: [i32; 64] = [
        4, 4, 155, 155, 29, 183, 183, 29, 4, 27, 27, 155, 29, 29, 29, 29, 183, 183, 183, 27, 155,
        155, 155, 155, 183, 183, 183, 27, 27, 155, 155, 27, 183, 183, 183, 183, 183, 27, 155, 5,
        183, 183, 183, 183, 183, 183, 183, 4, 183, 183, 183, 183, 183, 183, 183, 4, 177, 183, 183,
        183, 183, 183, 183, 183,
    ];
    const NETHER: [i32; 64] = [
        171, 171, 171, 171, 170, 170, 170, 170, 171, 171, 171, 171, 170, 170, 170, 170, 8, 170,
        171, 171, 170, 170, 170, 170, 8, 170, 170, 170, 170, 170, 170, 170, 8, 8, 170, 170, 170,
        170, 170, 170, 8, 8, 8, 170, 170, 170, 170, 170, 8, 8, 8, 170, 170, 170, 170, 170, 172, 8,
        8, 8, 170, 170, 170, 8,
    ];
    const OUTER_END: [i32; 64] = [
        40, 40, 43, 41, 42, 40, 43, 42, 40, 42, 41, 40, 40, 40, 43, 41, 40, 41, 43, 40, 40, 40, 40,
        41, 40, 40, 42, 40, 42, 41, 40, 42, 40, 40, 43, 43, 41, 40, 40, 42, 41, 43, 41, 42, 40, 43,
        43, 43, 42, 43, 41, 40, 40, 43, 41, 42, 42, 41, 40, 40, 41, 41, 42, 41,
    ];
    const LARGE: [i32; 64] = [
        45, 45, 48, 48, 45, 45, 45, 45, 45, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
        48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
        48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
    ];
    let cancel = AtomicBool::new(false);
    for version in ["1.20.1", "1.20.6", "1.21.1", "1.21.3"] {
        for (seed, dimension, y, expected) in [
            ("0", "overworld", 64, NORMAL),
            ("-9223372036854775808", "overworld", -32, CAVES),
            ("9223372036854775807", "nether", 64, NETHER),
        ] {
            let mut r = map(seed);
            r.version = version.into();
            r.dimension = dimension.into();
            r.y = y;
            let m = generate(&r, &cancel).unwrap();
            assert_eq!(m.biomes, expected, "{version}/{dimension}/{y}");
        }
        let mut end = map("9007199254740993");
        end.version = version.into();
        end.dimension = "end".into();
        end.x = 8192;
        end.z = -4096;
        assert_eq!(generate(&end, &cancel).unwrap().biomes, OUTER_END);
        let mut large = map("0");
        large.version = version.into();
        large.preset = "large_biomes".into();
        assert_eq!(generate(&large, &cancel).unwrap().biomes, LARGE);
    }
    let found = search(&query("village"), &cancel).unwrap();
    assert_eq!(
        found.matches.iter().map(|m| (m.x, m.z)).collect::<Vec<_>>(),
        vec![(608, 720), (272, 944), (544, -1056), (-1072, 1216)]
    );
    assert!(
        found
            .matches
            .iter()
            .all(|m| m.confidence == "biome_checked_terrain_unverified")
    );
}
#[test]
fn rejects_bad_inputs_and_unsupported_world_generation() {
    let cancel = AtomicBool::new(false);
    for seed in [
        "9223372036854775808",
        "-9223372036854775809",
        "1e6",
        "+1",
        "",
        "42.0",
        " 42",
    ] {
        assert_eq!(
            generate(&map(seed), &cancel).unwrap_err().code,
            "SEED_MAP_INVALID_SEED"
        );
    }
    for version in ["1.20.2", "1.21.4", "1.21", "latest", "1.21.1-forge"] {
        let mut r = map("0");
        r.version = version.into();
        assert_eq!(
            generate(&r, &cancel).unwrap_err().code,
            "SEED_MAP_UNSUPPORTED_VERSION"
        );
    }
    for preset in ["amplified", "custom", "flat", "unknown"] {
        let mut r = map("0");
        r.preset = preset.into();
        assert_eq!(
            generate(&r, &cancel).unwrap_err().code,
            "SEED_MAP_UNSUPPORTED_WORLDGEN"
        );
    }
    let mut r = map("0");
    r.x = i32::MAX;
    assert_eq!(
        generate(&r, &cancel).unwrap_err().code,
        "SEED_MAP_INVALID_REQUEST"
    );
    r = map("0");
    r.height = 129;
    assert_eq!(
        generate(&r, &cancel).unwrap_err().code,
        "SEED_MAP_INVALID_REQUEST"
    );
    let mut q = query("trial_chambers");
    q.version = "1.20.6".into();
    assert_eq!(
        search(&q, &cancel).unwrap_err().code,
        "SEED_MAP_UNSUPPORTED_TARGET"
    );
}
#[test]
fn cancellation_and_work_deadline() {
    let cancel = AtomicBool::new(true);
    assert_eq!(
        generate(&map("0"), &cancel).unwrap_err().code,
        "SEED_MAP_CANCELLED"
    );
    assert_eq!(
        search(&query("village"), &cancel).unwrap_err().code,
        "SEED_MAP_CANCELLED"
    );
    cancel.store(false, Ordering::Relaxed);
    assert_eq!(
        checkpoint(&cancel, Instant::now() - Duration::from_secs(6))
            .unwrap_err()
            .code,
        "SEED_MAP_BUDGET_EXCEEDED"
    );
}
#[test]
fn cancellation_interrupts_active_biome_work() {
    let mut q = query("plains");
    q.target.kind = "biome".into();
    q.radius = 8192;
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| search(&q, &cancel));
        std::thread::sleep(Duration::from_millis(5));
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            worker.join().unwrap().unwrap_err().code,
            "SEED_MAP_CANCELLED"
        );
    });
}
#[test]
fn numeric_biome_catalog_id_matches_canonical_name() {
    let cancel = AtomicBool::new(false);
    let mut named = query("plains");
    named.target.kind = "biome".into();
    let mut numeric = named.clone();
    numeric.target.id = "1".into();
    let positions = |result: SearchResult| {
        result
            .matches
            .into_iter()
            .map(|m| (m.x, m.z, m.distance, m.confidence))
            .collect::<Vec<_>>()
    };
    let expected = positions(search(&named, &cancel).unwrap());
    assert!(!expected.is_empty());
    assert_eq!(positions(search(&numeric, &cancel).unwrap()), expected);
    for id in ["-1", "256", "1.0", "01", "unknown_biome"] {
        numeric.target.id = id.into();
        assert_eq!(
            search(&numeric, &cancel).unwrap_err().code,
            "SEED_MAP_UNSUPPORTED_TARGET"
        );
    }
}
#[test]
fn catalog_and_dimension_restrictions_are_consistent() {
    let caps = capabilities();
    let trial = caps
        .structures
        .iter()
        .find(|s| s.id == "trial_chambers")
        .unwrap();
    assert_eq!(trial.supported_versions, vec!["1.21.1", "1.21.3"]);
    assert!(!caps.biomes.iter().any(|b| b.name == "pale_garden"));
    assert!(caps.biomes.iter().any(|b| b.name == "cherry_grove"));
    let cancel = AtomicBool::new(false);
    let mut q = query("fortress");
    assert_eq!(
        search(&q, &cancel).unwrap_err().code,
        "SEED_MAP_UNSUPPORTED_TARGET"
    );
    q.target.id = "stronghold".into();
    assert_eq!(
        search(&q, &cancel).unwrap_err().code,
        "SEED_MAP_UNSUPPORTED_TARGET"
    );
    let mut r = map("0");
    r.preset = "large_biomes".into();
    r.dimension = "nether".into();
    assert_eq!(
        generate(&r, &cancel).unwrap_err().code,
        "SEED_MAP_UNSUPPORTED_WORLDGEN"
    );
    r = map("0");
    r.y = 320;
    assert_eq!(
        generate(&r, &cancel).unwrap_err().code,
        "SEED_MAP_INVALID_REQUEST"
    );
    r = map("9007199254740993");
    r.dimension = "end".into();
    r.x = 8192;
    r.z = -4096;
    let exact = generate(&r, &cancel).unwrap().biomes;
    r.seed = "9007199254740992".into();
    assert_ne!(generate(&r, &cancel).unwrap().biomes, exact);
}
#[test]
fn maximum_tile_and_search_are_bounded() {
    let cancel = AtomicBool::new(false);
    let mut r = map("0");
    r.width = 128;
    r.height = 128;
    r.step = 16;
    let start = Instant::now();
    let result = generate(&r, &cancel).unwrap();
    assert_eq!(result.biomes.len(), 16384);
    assert!(
        result
            .biomes
            .iter()
            .all(|id| result.palette.iter().any(|b| b.id == *id))
    );
    println!("128x128 tile {:?}", start.elapsed());
    let mut q = query("plains");
    q.target.kind = "biome".into();
    q.radius = 8192;
    q.limit = 32;
    let start = Instant::now();
    let found = search(&q, &cancel).unwrap();
    assert_eq!(found.sampled_step, Some(64));
    assert!(found.matches.len() <= 32);
    assert!(
        found
            .matches
            .windows(2)
            .all(|w| w[0].distance <= w[1].distance)
    );
    assert!(
        found
            .matches
            .iter()
            .all(|m| m.x % 64 == 0 && m.z % 64 == 0 && m.distance <= 8192.0)
    );
    println!("8192 radius sampled search {:?}", start.elapsed());
}
#[test]
fn upstream_authored_old_and_new_reference_hashes() {
    // Adapted from Cubitect/cubiomes tests.c getRef/testGeneration at the
    // pinned revision. Copyright(c)2020 Cubitect; vendor/cubiomes/LICENSE.
    // Expected checksums are upstream-authored, not generated by this crate.
    fn hash32(mut x: u32) -> u32 {
        x ^= x >> 15;
        x = x.wrapping_mul(0xd168aaad);
        x ^= x >> 15;
        x = x.wrapping_mul(0xaf723597);
        x ^ (x >> 15)
    }
    for (version, expected) in [
        ("1.12.2", 0xbc75e996),
        ("1.16.5", 0xde9a6574),
        ("1.18.2", 0xade7f891),
        ("1.19.2", 0xea3e8c1c),
        ("1.19.4", 0x391c36ec),
        ("1.20.1", 0x0f8888ab),
        ("1.20.6", 0x0f8888ab),
    ] {
        let mut actual = 0u32;
        for x in -32i32..32 {
            for z in -32i32..32 {
                let seed = i64::from((z << 6) ^ x);
                let y = (((hash32(seed as u32) & 0x7fffffff) % 384) as i32 - 64) >> 2;
                let g =
                    ffi::Generator::new(seed, version_code(version).unwrap(), 0, false).unwrap();
                let id = g.upstream_biome4(x, y, z);
                actual ^= hash32((seed as u32) ^ ((id as u32) << 12));
            }
        }
        assert_eq!(actual, expected, "upstream b6 hash {version}");
    }
}
#[test]
fn version_scoped_catalogs_y_and_structure_boundaries() {
    let caps = capabilities();
    let serialized = serde_json::to_vec(&caps).unwrap();
    println!(
        "Version-scoped capabilities serialized bytes: {}",
        serialized.len()
    );
    assert!(serialized.len() < 256 * 1024);
    assert_eq!(caps.version_catalogs.len(), 11);
    for v in &caps.version_catalogs {
        let code = version_code(v.version).unwrap();
        assert!(
            v.biomes
                .iter()
                .filter(|b| b.dimension == "end")
                .map(|b| b.id)
                .collect::<Vec<_>>()
                == vec![9, 40, 41, 42, 43]
        );
        assert_eq!(v.y_bounds.overworld.min, if code < 11802 { 0 } else { -64 });
        let names = |name: &str| v.biomes.iter().any(|b| b.name == name);
        assert_eq!(names("cherry_grove"), code >= 12001);
        assert_eq!(names("lush_caves"), code >= 11802);
        assert_eq!(names("snowy_tundra"), code < 11802);
        assert_eq!(names("snowy_plains"), code >= 11802);
        assert!(!names("mountain_edge") && !names("deep_warm_ocean"));
        let has = |id: &str| v.structures.iter().any(|s| s.id == id);
        assert_eq!(has("shipwreck"), code > 11202);
        assert_eq!(has("ruined_portal"), code >= 11601);
        assert_eq!(has("ancient_city"), code >= 11902);
        assert_eq!(has("trail_ruins"), code >= 12001);
        assert_eq!(has("trial_chambers"), code >= 12101);
        // Exercise every explicit mapping and dimension, including legacy
        // layered generators and their version-scoped output palettes.
        let cancel = AtomicBool::new(false);
        for dimension in ["overworld", "nether", "end"] {
            let mut r = map("-9223372036854775808");
            r.version = v.version.into();
            r.dimension = dimension.into();
            r.x = 8192;
            r.z = -4096;
            let result = generate(&r, &cancel).unwrap();
            assert!(
                result
                    .biomes
                    .iter()
                    .all(|id| result.palette.iter().any(|b| b.id == *id))
            );
            assert!(result.palette.iter().all(|b| b.dimension == dimension));
        }
        let mut r = map("9223372036854775807");
        r.version = v.version.into();
        r.preset = "large_biomes".into();
        assert_eq!(generate(&r, &cancel).unwrap().biomes.len(), 64);
    }
    let family = caps
        .version_catalogs
        .iter()
        .find(|v| v.version == "1.20.1")
        .unwrap();
    assert_eq!((family.engine_version, family.mapping), ("1.20", "family"));
    let cancel = AtomicBool::new(false);
    for (version, id) in [
        ("1.12.2", "shipwreck"),
        ("1.16.5", "ancient_city"),
        ("1.19.4", "trail_ruins"),
        ("1.20.1", "trial_chambers"),
    ] {
        let mut q = query(id);
        q.version = version.into();
        assert_eq!(
            search(&q, &cancel).unwrap_err().code,
            "SEED_MAP_UNSUPPORTED_TARGET"
        );
    }
    let mut r = map("0");
    r.version = "1.16.5".into();
    r.y = -1;
    assert_eq!(
        generate(&r, &cancel).unwrap_err().code,
        "SEED_MAP_INVALID_REQUEST"
    );
    r.y = 255;
    let m = generate(&r, &cancel).unwrap();
    assert!(
        m.biomes
            .iter()
            .all(|id| m.palette.iter().any(|b| b.id == *id))
    );
}
#[test]
fn legacy_maximum_queries_measurement() {
    let cancel = AtomicBool::new(false);
    for version in ["1.12.2", "1.16.5"] {
        let mut r = map("0");
        r.version = version.into();
        r.width = 128;
        r.height = 128;
        r.step = 16;
        let start = Instant::now();
        let result = generate(&r, &cancel);
        println!(
            "legacy {version} 128x128 map {:?} {:?}",
            start.elapsed(),
            result.as_ref().map(|r| r.biomes.len())
        );
        assert!(result.is_ok());
        let mut q = query("plains");
        q.version = version.into();
        q.target.kind = "biome".into();
        q.radius = 8192;
        let start = Instant::now();
        let result = search(&q, &cancel);
        println!(
            "legacy {version} 8192-radius search {:?} {:?}",
            start.elapsed(),
            result.as_ref().map(|r| r.matches.len())
        );
        assert!(result.is_ok());
    }
}
