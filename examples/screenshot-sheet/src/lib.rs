// SPDX-License-Identifier: MIT
//! Pure layout of selected screenshot metadata, without access to pixels or files.
wit_bindgen::generate!({path:"../../contracts/image-layout",world:"image-layout"});
use exports::launcher::image_layout::layout::{
    Failure, Guest, Image, Ordering, Request, Rgb, Sheet,
};
use std::collections::BTreeSet;

struct ScreenshotSheet;
fn fail(locale: &str, code: &str, en: &str, zh: &str) -> Failure {
    Failure {
        code: code.into(),
        message: if locale == "zh-CN" { zh } else { en }.into(),
    }
}
fn plain(value: &str, max: usize) -> bool {
    value.len() <= max && !value.chars().any(char::is_control)
}
fn automatic_columns(images: &[Image]) -> u8 {
    // The host uses square contain-fit cells. Favor a canvas matching the selected
    // images' average orientation, with a penalty for empty cells on the last row.
    let target = images
        .iter()
        .map(|i| (f64::from(i.width) / f64::from(i.height)).clamp(0.5, 2.0))
        .sum::<f64>()
        / images.len() as f64;
    let mut best = (f64::INFINITY, 1);
    for columns in 1..=images.len().min(4) {
        let rows = images.len().div_ceil(columns);
        let empty = rows * columns - images.len();
        let score = (columns as f64 / rows as f64 - target).abs()
            + 0.75 * empty as f64 / images.len() as f64;
        if score < best.0 {
            best = (score, columns as u8);
        }
    }
    best.1
}
fn compose(request: Request) -> Result<Sheet, Failure> {
    if !matches!(request.locale.as_str(), "en" | "zh-CN")
        || request.images.is_empty()
        || request.images.len() > 12
        || request.columns > 4
    {
        return Err(fail(
            &request.locale,
            "IMAGE_LAYOUT_REQUEST",
            "Select 1–12 screenshots and use automatic layout or 1–4 columns.",
            "请选择 1–12 张截图，并使用自动布局或 1–4 列。",
        ));
    }
    let mut ids = BTreeSet::new();
    for image in &request.images {
        if image.id.is_empty()
            || !plain(&image.id, 128)
            || !image
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            || image.name.is_empty()
            || !plain(&image.name, 160)
            || image.name.contains(['/', '\\'])
            || !(1..=256).contains(&image.width)
            || !(1..=256).contains(&image.height)
            || !ids.insert(image.id.as_str())
        {
            return Err(fail(
                &request.locale,
                "IMAGE_LAYOUT_METADATA",
                "Refresh the gallery and select distinct screenshots with valid dimensions.",
                "请刷新图库，选择尺寸有效且不重复的截图。",
            ));
        }
    }
    let columns = if request.columns == 0 {
        automatic_columns(&request.images)
    } else {
        request.columns
    };
    let mut images = request.images;
    // Stable sort retains explicit selection order for equal dates. Unknown dates
    // are last in either direction instead of being mistaken for the oldest date.
    match request.ordering {
        Ordering::Selected => {}
        Ordering::Oldest => images.sort_by(|a, b| match (a.modified_at, b.modified_at) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }),
        Ordering::Newest => images.sort_by(|a, b| match (a.modified_at, b.modified_at) {
            (Some(a), Some(b)) => b.cmp(&a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }),
    }
    Ok(Sheet {
        columns,
        gap: 12,
        background: Rgb {
            red: 24,
            green: 26,
            blue: 30,
        },
        image_ids: images.into_iter().map(|image| image.id).collect(),
    })
}
impl Guest for ScreenshotSheet {
    fn compose(request: Request) -> Result<Sheet, Failure> {
        compose(request)
    }
}
export!(ScreenshotSheet);

#[cfg(test)]
mod tests {
    use super::*;
    fn image(id: &str, width: u16, height: u16, date: Option<u64>) -> Image {
        Image {
            id: id.into(),
            name: format!("{id}.png"),
            width,
            height,
            modified_at: date,
        }
    }
    fn request(images: Vec<Image>) -> Request {
        Request {
            locale: "en".into(),
            images,
            columns: 0,
            ordering: Ordering::Selected,
        }
    }
    #[test]
    fn mixed_original_fixture_preserves_selection_and_bounds() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/mixed-selection.json")).unwrap();
        let images = fixture["images"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| Image {
                id: v["id"].as_str().unwrap().into(),
                name: v["name"].as_str().unwrap().into(),
                width: v["width"].as_u64().unwrap() as u16,
                height: v["height"].as_u64().unwrap() as u16,
                modified_at: v["modified_at"].as_u64(),
            })
            .collect();
        let a = compose(request(images)).unwrap();
        assert_eq!(a.image_ids, ["landscape", "portrait", "square"]);
        assert!((1..=4).contains(&a.columns));
        assert_eq!(a.gap, 12);
        assert_eq!(
            (a.background.red, a.background.green, a.background.blue),
            (24, 26, 30)
        );
    }
    #[test]
    fn automatic_orientation_and_explicit_columns() {
        assert_eq!(
            compose(request(vec![
                image("a", 192, 108, None),
                image("b", 192, 108, None)
            ]))
            .unwrap()
            .columns,
            2
        );
        assert_eq!(
            compose(request(vec![
                image("a", 108, 192, None),
                image("b", 108, 192, None)
            ]))
            .unwrap()
            .columns,
            1
        );
        let mut r = request(vec![image("a", 256, 256, None)]);
        r.columns = 4;
        assert_eq!(compose(r).unwrap().columns, 4);
    }
    #[test]
    fn date_order_is_stable_and_unknown_dates_last() {
        for (ordering, expected) in [
            (Ordering::Oldest, vec!["old", "tie-a", "tie-b", "unknown"]),
            (Ordering::Newest, vec!["tie-a", "tie-b", "old", "unknown"]),
        ] {
            let mut r = request(vec![
                image("unknown", 256, 256, None),
                image("tie-a", 256, 256, Some(20)),
                image("old", 256, 256, Some(10)),
                image("tie-b", 256, 256, Some(20)),
            ]);
            r.ordering = ordering;
            assert_eq!(compose(r).unwrap().image_ids, expected);
        }
    }
    #[test]
    fn rejects_bad_metadata_and_localizes_failure() {
        let mut r = request(vec![image("a", 0, 10, None)]);
        r.locale = "zh-CN".into();
        assert!(compose(r).unwrap_err().message.contains("刷新"));
        assert!(
            compose(request(vec![
                image("a", 1, 1, None),
                image("a", 1, 1, None)
            ]))
            .is_err()
        );
        assert!(compose(request(vec![])).is_err());
        assert!(
            compose(request(
                (0..13).map(|n| image(&n.to_string(), 1, 1, None)).collect()
            ))
            .is_err()
        );
        let mut r = request(vec![image("a", 1, 1, None)]);
        r.columns = 5;
        assert!(compose(r).is_err());
        let mut r = request(vec![image("a", 1, 1, None)]);
        r.images[0].name = "bad\nname".into();
        assert!(compose(r).is_err());
    }
    #[test]
    fn every_bounded_selection_is_an_exact_deterministic_permutation() {
        for count in 1..=12 {
            let images: Vec<_> = (0..count)
                .map(|n| image(&format!("id{n}"), 192, 108, Some(n as u64)))
                .collect();
            let a = compose(request(images.clone())).unwrap();
            let b = compose(request(images)).unwrap();
            assert_eq!(a.columns, b.columns);
            assert_eq!(a.image_ids, b.image_ids);
            assert_eq!(a.image_ids.len(), count);
            assert_eq!(a.image_ids.iter().collect::<BTreeSet<_>>().len(), count);
            let rows = count.div_ceil(a.columns as usize);
            assert!(rows * 256 + (rows + 1) * a.gap as usize <= 2048);
        }
    }
}
