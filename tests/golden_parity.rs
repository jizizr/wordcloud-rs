//! Exact parity against golden data produced by the legacy Python code
//! (`tools/make_golden.py`).

use serde_json::Value;
use wordcloud_rs::rng::PyRandom;
use wordcloud_rs::text::{TextOptions, process_text};

fn golden(name: &str) -> Value {
    let path = format!("{}/tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn rng_streams_match_cpython() {
    for case in golden("rng.json").as_array().unwrap() {
        let mut r = PyRandom::new(case["seed"].as_i64().unwrap());
        for v in case["random"].as_array().unwrap() {
            assert_eq!(r.random(), v.as_f64().unwrap());
        }
        for (n, v) in [1i64, 2, 49, 255, 1000, 1 << 31, 7]
            .iter()
            .zip(case["randint"].as_array().unwrap())
        {
            assert_eq!(r.randint(0, *n), v.as_i64().unwrap());
        }
        for v in case["uniform"].as_array().unwrap() {
            assert_eq!(r.uniform(0.2, 1.0), v.as_f64().unwrap());
        }
        for (k, v) in [1u32, 7, 32, 33, 64]
            .iter()
            .zip(case["bits"].as_array().unwrap())
        {
            assert_eq!(r.getrandbits(*k), v.as_u64().unwrap());
        }
    }
}

#[test]
fn process_text_matches_python_including_order() {
    let mut mismatches = Vec::new();
    for case in golden("text.json").as_array().unwrap() {
        let corpus = case["corpus"].as_str().unwrap();
        let file = corpus.rsplit('/').next().unwrap();
        let text = std::fs::read_to_string(format!(
            "{}/tests/golden/{file}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let o = &case["opts"];
        let mut opts = TextOptions::default();
        if let Some(v) = o.get("collocations") {
            opts.collocations = v.as_bool().unwrap();
        }
        if let Some(v) = o.get("normalize_plurals") {
            opts.normalize_plurals = v.as_bool().unwrap();
        }
        if let Some(v) = o.get("include_numbers") {
            opts.include_numbers = v.as_bool().unwrap();
        }
        if let Some(v) = o.get("min_word_length") {
            opts.min_word_length = v.as_u64().unwrap() as usize;
        }
        if let Some(v) = o.get("collocation_threshold") {
            opts.collocation_threshold = v.as_f64().unwrap();
        }
        if let Some(v) = o.get("regexp") {
            opts.regexp = Some(v.as_str().unwrap().to_string());
        }
        let got: Vec<(String, i64)> = process_text(&text, &opts).unwrap().into_iter().collect();
        let want: Vec<(String, i64)> = case["counts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_i64().unwrap()))
            .collect();
        if got != want {
            let first = got.iter().zip(&want).position(|(a, b)| a != b);
            mismatches.push(format!(
                "{file} {o}: len {} vs {}, first diff at {first:?}",
                got.len(),
                want.len()
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
