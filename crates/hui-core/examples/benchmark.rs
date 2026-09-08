use hui_core::Document;
use std::time::Instant;

fn main() {
    for mb in [1, 10, 50] {
        let line = "# Performance 基准\r\n\r\nA paragraph with **Markdown**, 中文 and 👩‍💻.\r\n\r\n";
        let text = line.repeat(mb * 1024 * 1024 / line.len());
        let now = Instant::now();
        let mut doc = Document::new(&text);
        let open_ms = now.elapsed().as_secs_f64() * 1000.;
        let mut samples = Vec::new();
        for i in 0..200 {
            let pos = (i * 7919) % doc.len_chars();
            let start = Instant::now();
            let view = doc.viewport(pos, 8192);
            let next = format!("x{}", view.text);
            doc.replace_viewport(&view, &next).unwrap();
            std::hint::black_box(doc.is_dirty());
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{{\"bytes\":{},\"open_ms\":{open_ms:.3},\"viewport_edit_p95_ms\":{:.3},\"samples\":200}}",
            text.len(),
            samples[190]
        );
    }
}
