use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    io::stdout().lock().write_all(render(&input).as_bytes())
}

fn render(input: &str) -> String {
    let mut html = String::from("<pre class=\"terminal\"><code>");
    let mut chars = input.chars().peekable();
    let mut line_start = true;
    let mut style = Style::default();
    let mut rendered = Style::default();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            let mut parameters = String::new();
            for control in chars.by_ref() {
                if ('@'..='~').contains(&control) {
                    if control == 'm' {
                        style.apply(&parameters);
                    }
                    break;
                }
                parameters.push(control);
            }
            continue;
        }
        if style != rendered {
            rendered.close(&mut html);
            style.open(&mut html);
            rendered = style;
        }
        match ch {
            '$' if line_start && chars.peek() == Some(&' ') => {
                html.push_str("<span class=\"p\">$</span>");
            }
            '&' => html.push_str("&amp;"),
            '<' => html.push_str("&lt;"),
            '>' => html.push_str("&gt;"),
            _ => html.push(ch),
        }
        line_start = ch == '\n';
    }
    rendered.close(&mut html);
    html.push_str("</code></pre>\n");
    html
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    bold: bool,
    foreground: Option<&'static str>,
}

impl Style {
    fn apply(&mut self, parameters: &str) {
        let mut codes = parameters.split(';');
        while let Some(code) = codes.next() {
            match code {
                "" | "0" => *self = Self::default(),
                "1" => self.bold = true,
                "22" => self.bold = false,
                "31" => self.foreground = Some("r"),
                "32" => self.foreground = Some("g"),
                "33" => self.foreground = Some("y"),
                "36" => self.foreground = Some("c"),
                "30" | "34" | "35" | "37" | "39" => self.foreground = None,
                // Ignore extended colours as a unit, not as independent SGR codes.
                "38" | "48" | "58" => {
                    if code == "38" {
                        self.foreground = None;
                    }
                    let count = match codes.next() {
                        Some("5") => 1,
                        Some("2") => 3,
                        _ => break,
                    };
                    for _ in 0..count {
                        codes.next();
                    }
                }
                _ => {}
            }
        }
    }

    fn open(self, html: &mut String) {
        if self.bold {
            html.push_str("<b>");
        }
        if let Some(class) = self.foreground {
            html.push_str("<span class=\"");
            html.push_str(class);
            html.push_str("\">");
        }
    }

    fn close(self, html: &mut String) {
        if self.foreground.is_some() {
            html.push_str("</span>");
        }
        if self.bold {
            html.push_str("</b>");
        }
    }
}
