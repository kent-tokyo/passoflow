use std::hint::black_box;
use std::time::Instant;

use passoflow_capture::{CapturedFrame, ImageFrame, Origin};
use passoflow_vision::{SearchConfig, Template, search};

fn main() {
    let frame = CapturedFrame {
        origin: Origin::default(),
        image: ImageFrame::new(64, 64, vec![32; 64 * 64 * 4]).expect("fixture dimensions match"),
    };
    let template = Template {
        name: "solid".to_owned(),
        image: ImageFrame::new(8, 8, vec![32; 8 * 8 * 4]).expect("fixture dimensions match"),
    };
    let start = Instant::now();
    let iterations = 100;
    for _ in 0..iterations {
        black_box(search(
            black_box(&frame),
            black_box(std::slice::from_ref(&template)),
            None,
            SearchConfig::default(),
        ))
        .expect("fixed benchmark search should succeed");
    }
    let elapsed = start.elapsed();
    println!(
        "passoflow-vision exact-search: {iterations} iterations in {:?} ({:?}/iteration)",
        elapsed,
        elapsed / iterations
    );
}
