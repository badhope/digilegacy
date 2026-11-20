//! Seeded fuzz smoke for the HTML escaping boundary.
//!
//! Reports and exports interpolate user values into a printable shell. A raw
//! `<`, `>`, quote or apostrophe reaching the output would turn a title into
//! markup, so the property is checked across arbitrary Unicode text.

use digilegacy_render::{escape_html, render_document, render_page, Document};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const SEED: u64 = 0xD161_E65E;
const CASES: usize = 5_000;
const CHARS: &[char] = &[
    'a', 'Z', '0', '&', '<', '>', '"', '\'', '/', '=', ' ', '\t', '\r', '\n', '\0', '中', '文',
    'é', '🧪',
];

fn text(rng: &mut StdRng, max_chars: usize) -> String {
    let len = rng.random_range(0..=max_chars);
    (0..len)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())])
        .collect()
}

fn assert_no_raw_markup(value: &str) {
    for forbidden in ['<', '>', '"', '\''] {
        assert!(
            !value.contains(forbidden),
            "escaped text leaked {forbidden:?}: {value:?}"
        );
    }
}

#[test]
fn fuzz_html_escaping_never_leaks_markup() {
    let mut rng = StdRng::seed_from_u64(SEED);

    for payload in [
        "<script>alert(1)</script>",
        "&\"'<>",
        "\0<div>\r\n",
        "中文<script>🧪</script>",
    ] {
        assert_no_raw_markup(&escape_html(payload));
    }

    for _ in 0..CASES {
        let input = text(&mut rng, 80);
        let escaped = escape_html(&input);
        assert_no_raw_markup(&escaped);

        let page = render_page(&input, "<p>trusted body</p>", "en-US");
        assert!(
            page.contains(&escaped),
            "page title was not escaped: {input:?}"
        );
        assert!(page.contains("<p>trusted body</p>"));

        let document = render_document(&Document {
            title: input.clone(),
            body: input,
        });
        assert!(
            !document.contains("<script>"),
            "document leaked a script tag"
        );
    }
}
