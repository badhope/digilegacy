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

/// Render a trusted HTML body inside a minimal printable shell.
/// Callers must escape every user value before interpolation. This function
/// exists so exports can have headings/tables without turning data into markup.
pub fn render_page(title: &str, body: &str, locale: &str) -> String {
    let lang = if locale == "en-US" { "en" } else { "zh-CN" };
    format!(
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font-family:system-ui,-apple-system,'Noto Sans SC',sans-serif;max-width:920px;margin:40px auto;padding:0 24px;line-height:1.7;color:#20251f}}h1{{font-size:30px}}h2{{margin-top:32px;border-bottom:1px solid #ddd;padding-bottom:6px}}.muted{{color:#6d7268}}.card{{border:1px solid #ddd;border-radius:8px;padding:14px 16px;margin:10px 0}}.mono{{font-family:ui-monospace,monospace;font-size:12px}}li{{margin:6px 0}}</style></head><body><main>{body}</main></body></html>",
        escape_html(title)
    )
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
    fn page_keeps_trusted_body_and_escapes_title() {
        let html = render_page("<x>", "<h2>ok</h2>", "zh-CN");
        assert!(html.contains("&lt;x&gt;"));
        assert!(html.contains("<h2>ok</h2>"));
        assert!(html.contains("lang=\"zh-CN\""));
    }

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
