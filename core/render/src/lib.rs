//! Pure render layer (RND-01..04).
//!
//! Renderers take DTOs and return strings. They do not read the database, write
//! files or call the network. This gives report/checklist/legal/FAQ one shell.

#![forbid(unsafe_code)]

pub struct Document {
    pub title: String,
    pub body: String,
}

pub fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn render_document(document: &Document) -> String {
    format!(
        "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>{}</title></head><body><main>{}</main></body></html>",
        escape_html(&document.title),
        escape_html(&document.body)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_untrusted_text() {
        let document = Document {
            title: "<script>".into(),
            body: "<b>ok</b>".into(),
        };
        let html = render_document(&document);
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&lt;b&gt;ok&lt;/b&gt;"));
        assert!(!html.contains("<script>"));
    }
}
