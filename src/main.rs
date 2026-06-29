use std::io::{self, Read};

#[derive(Debug, Clone)]
struct FormatOptions {
    heading_to_body_spacing: usize,    // 0 or 1, default is 1
    heading_to_heading_spacing: usize, // 0 or 1, default is 1
    wrap_paragraphs: bool,             // default is false
    wrap_lists: bool,                  // default is false
    fill_column: usize,                // default is 80
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            heading_to_body_spacing: 1,
            heading_to_heading_spacing: 1,
            wrap_paragraphs: false,
            wrap_lists: false,
            fill_column: 80,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    Normal,
    InBlock(String), // Closing tag string, e.g. "#+END_SRC"
    InDrawer { indent: String },
}

fn is_heading_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('*') {
        return false;
    }
    let stars_count = trimmed.chars().take_while(|&c| c == '*').count();
    if stars_count == 0 {
        return false;
    }
    let after_stars = &trimmed[stars_count..];
    if !(after_stars.starts_with(' ') || after_stars.is_empty()) {
        return false;
    }

    // In Org-mode, a line starting with exactly one star and having leading spaces
    // is a list item bullet, not a heading. Headings must start at column 0
    // or have multiple stars (e.g., "** subheading").
    let has_leading_spaces = line.starts_with(' ') || line.starts_with('\t');
    if has_leading_spaces && stars_count == 1 {
        return false;
    }

    true
}

fn get_heading_level(line: &str) -> Option<usize> {
    if !line.starts_with('*') {
        return None;
    }
    let stars_count = line.chars().take_while(|&c| c == '*').count();
    if stars_count == 0 {
        return None;
    }
    let after_stars = &line[stars_count..];
    if after_stars.starts_with(' ') || after_stars.is_empty() {
        Some(stars_count)
    } else {
        None
    }
}

fn is_heading_or_drawer_end(line: &str) -> bool {
    is_heading_line(line) || line.trim().ends_with(":END:")
}

fn is_list_item(line: &str) -> bool {
    let trimmed = line.trim_start();

    // Unordered bullets: -, +, *
    if trimmed.starts_with("-") || trimmed.starts_with("+") || trimmed.starts_with("*") {
        let after_bullet = &trimmed[1..];
        if after_bullet.starts_with(' ')
            || after_bullet.starts_with('\t')
            || after_bullet.is_empty()
        {
            return true;
        }
    }

    // Ordered bullets: digit followed by . or )
    if let Some(first_char) = trimmed.chars().next() {
        if first_char.is_ascii_digit() {
            let digits_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
            let after_digits = &trimmed[digits_count..];
            if after_digits.starts_with(".") || after_digits.starts_with(")") {
                let after_marker = &after_digits[1..];
                if after_marker.starts_with(' ')
                    || after_marker.starts_with('\t')
                    || after_marker.is_empty()
                {
                    return true;
                }
            }
        } else if first_char.is_ascii_lowercase() {
            // Check for alphabetical list markers like "a. " or "a) "
            let after_char = &trimmed[1..];
            if after_char.starts_with(".") || after_char.starts_with(")") {
                let after_marker = &after_char[1..];
                if after_marker.starts_with(' ')
                    || after_marker.starts_with('\t')
                    || after_marker.is_empty()
                {
                    return true;
                }
            }
        }
    }
    false
}

fn format_list_line(line: &str) -> String {
    let indent = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect::<String>();
    let trimmed = line.trim_start();
    if trimmed.starts_with("-") {
        let content = trimmed[1..].trim();
        if content.is_empty() {
            format!("{}-", indent)
        } else {
            format!("{}- {}", indent, content)
        }
    } else if trimmed.starts_with("+") {
        let content = trimmed[1..].trim();
        if content.is_empty() {
            format!("{}+", indent)
        } else {
            format!("{}+ {}", indent, content)
        }
    } else if trimmed.starts_with("*") {
        let content = trimmed[1..].trim();
        if content.is_empty() {
            format!("{}*", indent)
        } else {
            format!("{}* {}", indent, content)
        }
    } else {
        let digits_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits_count > 0 {
            let after_digits = &trimmed[digits_count..];
            if after_digits.starts_with('.') || after_digits.starts_with(')') {
                let marker = &trimmed[..digits_count + 1];
                let content = trimmed[digits_count + 1..].trim();
                if content.is_empty() {
                    format!("{}{}", indent, marker)
                } else {
                    format!("{}{} {}", indent, marker, content)
                }
            } else {
                line.to_string()
            }
        } else {
            if let Some(first_char) = trimmed.chars().next() {
                if first_char.is_ascii_lowercase() {
                    let after_char = &trimmed[1..];
                    if after_char.starts_with('.') || after_char.starts_with(')') {
                        let marker = &trimmed[..2];
                        let content = trimmed[2..].trim();
                        if content.is_empty() {
                            format!("{}{}", indent, marker)
                        } else {
                            format!("{}{} {}", indent, marker, content)
                        }
                    } else {
                        line.to_string()
                    }
                } else {
                    line.to_string()
                }
            } else {
                line.to_string()
            }
        }
    }
}

fn is_regular_paragraph_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return false;
    }

    // Check for structural headings
    if is_heading_line(line) {
        return false;
    }

    // Check for list items
    if is_list_item(trimmed) {
        return false;
    }

    // Check for drawers (starts and ends with : and is not end)
    if trimmed.starts_with(':') && trimmed.ends_with(':') {
        return false;
    }
    if trimmed.to_uppercase() == ":END:" {
        return false;
    }

    // Check for block boundaries
    let trimmed_upper = trimmed.to_uppercase();
    if trimmed_upper.starts_with("#+BEGIN_") || trimmed_upper.starts_with("#+END_") {
        return false;
    }

    // Check for metadata/keyword lines
    if trimmed.starts_with("#+") {
        return false;
    }

    // Check for comments
    if trimmed.starts_with('#') {
        return false;
    }

    // Check for tables
    if trimmed.starts_with('|') {
        return false;
    }

    true
}

fn char_width(c: char) -> usize {
    if c == '\t' {
        8
    } else {
        let val = c as u32;
        if (val >= 0x1100 && val <= 0x11FF) || // Hangul Jamo
           (val >= 0x2E80 && val <= 0x2EFF) || // CJK Radicals Supplement
           (val >= 0x3000 && val <= 0x303F) || // CJK Symbols and Punctuation
           (val >= 0x3040 && val <= 0x309F) || // Hiragana
           (val >= 0x30A0 && val <= 0x30FF) || // Katakana
           (val >= 0x3130 && val <= 0x318F) || // Hangul Compatibility Jamo
           (val >= 0x3200 && val <= 0x32FF) || // Enclosed CJK Letters and Months
           (val >= 0x3400 && val <= 0x4DBF) || // CJK Unified Ideographs Extension A
           (val >= 0x4E00 && val <= 0x9FFF) || // CJK Unified Ideographs
           (val >= 0xF900 && val <= 0xFAFF) || // CJK Compatibility Ideographs
           (val >= 0xFF00 && val <= 0xFFEF) || // Halfwidth and Fullwidth Forms
           (val >= 0xAC00 && val <= 0xD7AF) || // Hangul Syllables
           (val >= 0x20000 && val <= 0x3FFFD)   // Plane 2 & 3 CJK Ideographs
        {
            2
        } else {
            1
        }
    }
}

fn str_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

fn split_list_line(line: &str) -> Option<(String, String, String)> {
    let indent = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect::<String>();
    let trimmed = line.trim_start();
    
    if !is_list_item(trimmed) {
        return None;
    }

    if trimmed.starts_with('-') || trimmed.starts_with('+') || trimmed.starts_with('*') {
        let bullet_char = trimmed.chars().next().unwrap();
        let after_bullet = &trimmed[1..];
        let spaces = after_bullet
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect::<String>();
        let bullet_part = if spaces.is_empty() {
            format!("{}", bullet_char)
        } else {
            format!("{}{}", bullet_char, spaces)
        };
        let content = after_bullet[spaces.len()..].trim().to_string();
        return Some((indent, bullet_part, content));
    }

    if let Some(first_char) = trimmed.chars().next() {
        if first_char.is_ascii_digit() {
            let digits_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
            let after_digits = &trimmed[digits_count..];
            if after_digits.starts_with('.') || after_digits.starts_with(')') {
                let bullet_marker = &trimmed[..digits_count + 1];
                let after_marker = &trimmed[digits_count + 1..];
                let spaces = after_marker
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect::<String>();
                let bullet_part = format!("{}{}", bullet_marker, spaces);
                let content = after_marker[spaces.len()..].trim().to_string();
                return Some((indent, bullet_part, content));
            }
        } else if first_char.is_ascii_lowercase() {
            let after_char = &trimmed[1..];
            if after_char.starts_with('.') || after_char.starts_with(')') {
                let bullet_marker = &trimmed[..2];
                let after_marker = &trimmed[2..];
                let spaces = after_marker
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect::<String>();
                let bullet_part = format!("{}{}", bullet_marker, spaces);
                let content = after_marker[spaces.len()..].trim().to_string();
                return Some((indent, bullet_part, content));
            }
        }
    }

    None
}

fn wrap_list_line(line: &str, fill_column: usize) -> Vec<String> {
    if let Some((indent, bullet, content)) = split_list_line(line) {
        if content.is_empty() {
            return vec![format_list_line(line)];
        }

        let words: Vec<&str> = content.split_whitespace().collect();
        if words.is_empty() {
            return vec![format_list_line(line)];
        }

        let indent_width = str_width(&indent);
        let bullet_width = str_width(&bullet);
        let first_line_prefix_width = indent_width + bullet_width;

        let subsequent_indent = format!("{}{}", indent, " ".repeat(bullet_width));
        let subsequent_indent_width = indent_width + bullet_width;

        let mut wrapped_lines = Vec::new();
        let mut current_line = String::new();

        let first_target_width = if fill_column > first_line_prefix_width {
            fill_column - first_line_prefix_width
        } else {
            1
        };

        let subsequent_target_width = if fill_column > subsequent_indent_width {
            fill_column - subsequent_indent_width
        } else {
            1
        };

        let mut is_first_line = true;

        for word in words {
            let word_width = str_width(word);
            let current_width = str_width(&current_line);
            let target_width = if is_first_line { first_target_width } else { subsequent_target_width };

            if current_line.is_empty() {
                current_line.push_str(word);
            } else if current_width + 1 + word_width <= target_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                if is_first_line {
                    wrapped_lines.push(format!("{}{}{}", indent, bullet, current_line));
                    is_first_line = false;
                } else {
                    wrapped_lines.push(format!("{}{}", subsequent_indent, current_line));
                }
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            if is_first_line {
                wrapped_lines.push(format!("{}{}{}", indent, bullet, current_line));
            } else {
                wrapped_lines.push(format!("{}{}", subsequent_indent, current_line));
            }
        }

        wrapped_lines
    } else {
        vec![line.to_string()]
    }
}

fn wrap_paragraph(paragraph: &[String], fill_column: usize) -> Vec<String> {
    if paragraph.is_empty() {
        return Vec::new();
    }

    let first_line = &paragraph[0];
    let indent = first_line
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect::<String>();
    let indent_len = indent.len();

    // Clean the text by stripping the common indentation
    let cleaned_lines: Vec<String> = paragraph
        .iter()
        .map(|line| {
            if line.starts_with(&indent) {
                line[indent_len..].to_string()
            } else {
                line.trim_start().to_string()
            }
        })
        .collect();

    let combined_text = cleaned_lines.join(" ");
    let words: Vec<&str> = combined_text.split_whitespace().collect();

    if words.is_empty() {
        return Vec::new();
    }

    let mut wrapped_lines = Vec::new();
    let mut current_line = String::new();

    let indent_width = str_width(&indent);
    // Ensure target column accounts for indentation
    let target_width = if fill_column > indent_width {
        fill_column - indent_width
    } else {
        1 // Fallback in case indentation is larger than fill_column
    };

    for word in words {
        let word_width = str_width(word);
        let current_width = str_width(&current_line);
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_width + 1 + word_width <= target_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            wrapped_lines.push(format!("{}{}", indent, current_line));
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        wrapped_lines.push(format!("{}{}", indent, current_line));
    }

    wrapped_lines
}

fn format_org(input: &str, options: &FormatOptions) -> String {
    let mut lines: Vec<&str> = input.lines().collect();

    // 1. Trim leading and trailing empty lines from the document boundaries
    while !lines.is_empty() && lines[0].trim().is_empty() {
        lines.remove(0);
    }
    while !lines.is_empty() && lines[lines.len() - 1].trim().is_empty() {
        lines.pop();
    }

    if lines.is_empty() {
        return String::new();
    }

    let mut formatted_lines = Vec::new();
    let mut state = State::Normal;
    let mut pending_empty_lines = 0;
    let mut is_first_line = true;

    // Consecutive paragraph lines accumulator
    let mut accumulated_paragraph: Vec<String> = Vec::new();

    let flush_paragraph = |accumulated: &mut Vec<String>,
                           formatted: &mut Vec<String>,
                           pending_lines: &mut usize,
                           is_first: &mut bool| {
        if accumulated.is_empty() {
            return;
        }

        if !*is_first {
            for _ in 0..*pending_lines {
                formatted.push(String::new());
            }
        }
        *pending_lines = 0;

        let wrapped = wrap_paragraph(accumulated, options.fill_column);
        for line in wrapped {
            formatted.push(line);
        }

        accumulated.clear();
        *is_first = false;
    };

    for raw_line in lines {
        let trimmed = raw_line.trim_end();
        let is_empty = trimmed.trim().is_empty();

        match state {
            State::InBlock(ref closing_tag) => {
                let trimmed_upper = trimmed.trim().to_uppercase();
                if trimmed_upper.starts_with(closing_tag) {
                    let indent = raw_line
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>();
                    formatted_lines.push(format!("{}{}", indent, closing_tag));
                    state = State::Normal;
                    pending_empty_lines = 1;
                } else {
                    formatted_lines.push(trimmed.to_string());
                }
            }
            State::InDrawer { ref indent } => {
                let trimmed_upper = trimmed.trim().to_uppercase();
                if trimmed_upper == ":END:" {
                    formatted_lines.push(format!("{}{}", indent, ":END:"));
                    state = State::Normal;
                    pending_empty_lines = options.heading_to_body_spacing;
                } else if is_empty {
                    // Collapse empty lines inside drawers
                } else {
                    let inner_trimmed = trimmed.trim_start();
                    if inner_trimmed.starts_with(':') {
                        if let Some(colon_idx) = inner_trimmed[1..].find(':') {
                            let prop_name = &inner_trimmed[..colon_idx + 2];
                            let prop_value = inner_trimmed[colon_idx + 2..].trim();
                            if prop_value.is_empty() {
                                formatted_lines.push(format!("{}{}", indent, prop_name));
                            } else {
                                formatted_lines
                                    .push(format!("{}{} {}", indent, prop_name, prop_value));
                            }
                        } else {
                            formatted_lines.push(format!("{}{}", indent, inner_trimmed));
                        }
                    } else {
                        formatted_lines.push(format!("{}{}", indent, inner_trimmed));
                    }
                }
            }
            State::Normal => {
                if is_empty {
                    flush_paragraph(
                        &mut accumulated_paragraph,
                        &mut formatted_lines,
                        &mut pending_empty_lines,
                        &mut is_first_line,
                    );
                    pending_empty_lines = 1;
                    continue;
                }

                let trimmed_upper = trimmed.trim().to_uppercase();

                if trimmed_upper.starts_with("#+BEGIN_") {
                    flush_paragraph(
                        &mut accumulated_paragraph,
                        &mut formatted_lines,
                        &mut pending_empty_lines,
                        &mut is_first_line,
                    );

                    let last_is_header = formatted_lines
                        .last()
                        .map(|l| is_heading_or_drawer_end(l))
                        .unwrap_or(false);
                    if last_is_header {
                        pending_empty_lines = options.heading_to_body_spacing;
                    }

                    if !is_first_line {
                        for _ in 0..pending_empty_lines {
                            formatted_lines.push(String::new());
                        }
                    }
                    pending_empty_lines = 0;

                    let first_word_upper = trimmed_upper.split_whitespace().next().unwrap_or("");
                    let keyword = first_word_upper.strip_prefix("#+BEGIN_").unwrap_or("");
                    let closing_tag = format!("#+END_{}", keyword);

                    let indent = raw_line
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>();
                    let first_word = trimmed.split_whitespace().next().unwrap_or("");
                    let type_rest = trimmed.strip_prefix(first_word).unwrap_or("").trim();
                    if type_rest.is_empty() {
                        formatted_lines.push(format!("{}{}", indent, first_word.to_uppercase()));
                    } else {
                        let mut words = type_rest.split_whitespace();
                        if let Some(block_type) = words.next() {
                            let block_type_lower = block_type.to_lowercase();
                            let rest = type_rest.strip_prefix(block_type).unwrap_or("").trim();
                            if rest.is_empty() {
                                formatted_lines.push(format!(
                                    "{}{}{} {}",
                                    indent,
                                    first_word.to_uppercase(),
                                    "",
                                    block_type_lower
                                ));
                            } else {
                                formatted_lines.push(format!(
                                    "{}{}{} {} {}",
                                    indent,
                                    first_word.to_uppercase(),
                                    "",
                                    block_type_lower,
                                    rest
                                ));
                            }
                        } else {
                            formatted_lines.push(format!(
                                "{}{}{} {}",
                                indent,
                                first_word.to_uppercase(),
                                "",
                                type_rest
                            ));
                        }
                    }
                    state = State::InBlock(closing_tag);
                    is_first_line = false;
                } else if trimmed_upper.starts_with(":")
                    && trimmed_upper.ends_with(":")
                    && trimmed_upper != ":END:"
                {
                    flush_paragraph(
                        &mut accumulated_paragraph,
                        &mut formatted_lines,
                        &mut pending_empty_lines,
                        &mut is_first_line,
                    );

                    let last_is_header = formatted_lines
                        .last()
                        .map(|l| is_heading_or_drawer_end(l))
                        .unwrap_or(false);

                    let mut indent = raw_line
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .collect::<String>();

                    if last_is_header {
                        pending_empty_lines = 0;
                        if !indent.is_empty() {
                            if let Some(last_line) = formatted_lines.last() {
                                if let Some(level) = get_heading_level(last_line) {
                                    indent = " ".repeat(level + 1);
                                }
                            }
                        }
                    } else if !is_first_line {
                        for _ in 0..pending_empty_lines {
                            formatted_lines.push(String::new());
                        }
                        pending_empty_lines = 0;
                    }

                    formatted_lines.push(format!("{}{}", indent, trimmed_upper));
                    state = State::InDrawer { indent };
                    is_first_line = false;
                } else if is_heading_line(raw_line) {
                    flush_paragraph(
                        &mut accumulated_paragraph,
                        &mut formatted_lines,
                        &mut pending_empty_lines,
                        &mut is_first_line,
                    );

                    let last_is_header = formatted_lines
                        .last()
                        .map(|l| is_heading_or_drawer_end(l))
                        .unwrap_or(false);

                    let spacing = if last_is_header {
                        options.heading_to_heading_spacing
                    } else {
                        if is_first_line { 0 } else { 1 }
                    };

                    for _ in 0..spacing {
                        formatted_lines.push(String::new());
                    }
                    pending_empty_lines = 0;
                    formatted_lines.push(trimmed.trim_start().to_string());
                    is_first_line = false;
                } else if is_list_item(trimmed) {
                    flush_paragraph(
                        &mut accumulated_paragraph,
                        &mut formatted_lines,
                        &mut pending_empty_lines,
                        &mut is_first_line,
                    );

                    let last_is_header = formatted_lines
                        .last()
                        .map(|l| is_heading_or_drawer_end(l))
                        .unwrap_or(false);
                    if last_is_header {
                        pending_empty_lines = options.heading_to_body_spacing;
                    }

                    if !is_first_line {
                        for _ in 0..pending_empty_lines {
                            formatted_lines.push(String::new());
                        }
                    }
                    pending_empty_lines = 0;

                    if options.wrap_lists {
                        let wrapped = wrap_list_line(trimmed, options.fill_column);
                        for line in wrapped {
                            formatted_lines.push(line);
                        }
                    } else {
                        formatted_lines.push(format_list_line(trimmed));
                    }
                    is_first_line = false;
                } else {
                    if options.wrap_paragraphs && is_regular_paragraph_line(raw_line) {
                        if accumulated_paragraph.is_empty() {
                            let last_is_header = formatted_lines
                                .last()
                                .map(|l| is_heading_or_drawer_end(l))
                                .unwrap_or(false);
                            if last_is_header {
                                pending_empty_lines = options.heading_to_body_spacing;
                            }
                        }
                        accumulated_paragraph.push(trimmed.to_string());
                    } else {
                        flush_paragraph(
                            &mut accumulated_paragraph,
                            &mut formatted_lines,
                            &mut pending_empty_lines,
                            &mut is_first_line,
                        );

                        let last_is_header = formatted_lines
                            .last()
                            .map(|l| is_heading_or_drawer_end(l))
                            .unwrap_or(false);
                        if last_is_header {
                            pending_empty_lines = options.heading_to_body_spacing;
                        }

                        if !is_first_line {
                            for _ in 0..pending_empty_lines {
                                formatted_lines.push(String::new());
                            }
                        }
                        pending_empty_lines = 0;
                        formatted_lines.push(trimmed.to_string());
                        is_first_line = false;
                    }
                }
            }
        }
    }

    // Flush any remaining accumulated paragraph at the end of the file
    flush_paragraph(
        &mut accumulated_paragraph,
        &mut formatted_lines,
        &mut pending_empty_lines,
        &mut is_first_line,
    );

    formatted_lines.join("\n") + "\n"
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let check_mode = args.contains(&"--check".to_string());

    let mut options = FormatOptions::default();
    for arg in &args {
        if arg.starts_with("--heading-to-body-spacing=") {
            if let Some(val_str) = arg.split('=').nth(1) {
                if let Ok(val) = val_str.parse::<usize>() {
                    options.heading_to_body_spacing = val;
                }
            }
        }
        if arg.starts_with("--heading-to-heading-spacing=") {
            if let Some(val_str) = arg.split('=').nth(1) {
                if let Ok(val) = val_str.parse::<usize>() {
                    options.heading_to_heading_spacing = val;
                }
            }
        }
        if arg == "--wrap-paragraphs" {
            options.wrap_paragraphs = true;
        }
        if arg == "--wrap-lists" {
            options.wrap_lists = true;
        }
        if arg.starts_with("--fill-column=") {
            if let Some(val_str) = arg.split('=').nth(1) {
                if let Ok(val) = val_str.parse::<usize>() {
                    options.fill_column = val;
                }
            }
        }
    }

    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        eprintln!("Error reading stdin: {}", e);
        std::process::exit(1);
    }

    let formatted = format_org(&input, &options);

    if check_mode {
        if formatted != input {
            eprintln!("File is not formatted!");
            std::process::exit(1);
        } else {
            println!("File is formatted!");
            std::process::exit(0);
        }
    } else {
        print!("{}", formatted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_spacing() {
        let input = "
* Heading 1

Some text.


** Heading 2
:PROPERTIES:
:CUSTOM_ID: 123
:END:
More text.
";
        let expected = "* Heading 1

Some text.

** Heading 2
:PROPERTIES:
:CUSTOM_ID: 123
:END:

More text.
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_list_items() {
        let input = "
-    Item 1 with lots of spaces
- Item 2
+ Item 3
  1.    Ordered list item
";
        let expected = "- Item 1 with lots of spaces
- Item 2
+ Item 3
  1. Ordered list item
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_block_preservation() {
        let input = "
#+begin_src rust
fn main() {
    println!(\"Hello World\");


    // consecutive spaces are kept here!
}
#+end_src
";
        let expected = "#+BEGIN_SRC rust
fn main() {
    println!(\"Hello World\");


    // consecutive spaces are kept here!
}
#+END_SRC
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_enhanced_drawer_formatting() {
        let input = "* Heading 1
  :PROPERTIES:
  :CUSTOM_ID:     456
  :ANOTHER_PROP:  abc
  :END:
Some text.";
        let expected = "* Heading 1
  :PROPERTIES:
  :CUSTOM_ID: 456
  :ANOTHER_PROP: abc
  :END:

Some text.
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_heading_leading_spaces() {
        let input = "   ** Subheading 1.1
More text.";
        let expected = "** Subheading 1.1

More text.
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_heading_followed_by_text_no_newline() {
        let input = "* Heading 1
Some body text.";
        let expected = "* Heading 1

Some body text.
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_block_type_lowercase() {
        let input = "#+BEGIN_SRC RUST :tangle yes
fn main() {}
#+END_SRC";
        let expected = "#+BEGIN_SRC rust :tangle yes
fn main() {}
#+END_SRC
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_list_items_alphabetical_and_stars() {
        let input = "
  - item 1
  * item 2
  a.   alphabetical 1
  b)   alphabetical 2
";
        let expected = "  - item 1
  * item 2
  a. alphabetical 1
  b) alphabetical 2
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_custom_heading_to_body_spacing_0() {
        let input = "* Heading 1

Some text.
** Heading 2
Some text.";
        let options = FormatOptions {
            heading_to_body_spacing: 0,
            heading_to_heading_spacing: 1,
            ..FormatOptions::default()
        };
        let expected = "* Heading 1
Some text.

** Heading 2
Some text.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_custom_heading_to_heading_spacing_0() {
        let input = "* Heading 1

** Heading 2
Some text.";
        let options = FormatOptions {
            heading_to_body_spacing: 1,
            heading_to_heading_spacing: 0,
            ..FormatOptions::default()
        };
        let expected = "* Heading 1
** Heading 2

Some text.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_wrap_paragraphs_disabled_by_default() {
        let input = "This is a long line that should not be wrapped because paragraph wrapping is disabled by default.";
        let expected = "This is a long line that should not be wrapped because paragraph wrapping is disabled by default.
";
        assert_eq!(format_org(input, &FormatOptions::default()), expected);
    }

    #[test]
    fn test_wrap_paragraphs_enabled() {
        let input = "This is a long paragraph that should be wrapped to at most forty characters width. It has multiple words.";
        let options = FormatOptions {
            wrap_paragraphs: true,
            fill_column: 40,
            ..FormatOptions::default()
        };
        let expected = "This is a long paragraph that should be
wrapped to at most forty characters
width. It has multiple words.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_wrap_indented_paragraphs() {
        let input = "  This is an indented paragraph that has
  multiple lines and should preserve
  the two spaces indentation.";
        let options = FormatOptions {
            wrap_paragraphs: true,
            fill_column: 50,
            ..FormatOptions::default()
        };
        let expected = "  This is an indented paragraph that has multiple
  lines and should preserve the two spaces
  indentation.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_custom_heading_to_body_spacing_2() {
        let input = "* Heading 1
Some body text.
** Heading 2
Some other body text.";
        let options = FormatOptions {
            heading_to_body_spacing: 2,
            ..FormatOptions::default()
        };
        let expected = "* Heading 1


Some body text.

** Heading 2


Some other body text.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_custom_heading_to_body_spacing_with_drawer_and_wrap() {
        let input = "* Heading 1
  :PROPERTIES:
  :CUSTOM_ID: abc
  :END:
This is some body text that will be wrapped because wrap paragraphs is enabled and it is quite long.
* Heading 2
This is some other body text under heading 2.";
        let options = FormatOptions {
            heading_to_body_spacing: 2,
            wrap_paragraphs: true,
            fill_column: 40,
            ..FormatOptions::default()
        };
        let expected = "* Heading 1
  :PROPERTIES:
  :CUSTOM_ID: abc
  :END:


This is some body text that will be
wrapped because wrap paragraphs is
enabled and it is quite long.

* Heading 2


This is some other body text under
heading 2.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_wrap_paragraphs_korean() {
        let input = "오알지포매터는 이맥스의 기능을 러스트로 구현하여 빠르게 문서를 포맷팅할 수 있는 아주 유용한 도구입니다.";
        let options = FormatOptions {
            wrap_paragraphs: true,
            fill_column: 40,
            ..FormatOptions::default()
        };
        let expected = "오알지포매터는 이맥스의 기능을 러스트로
구현하여 빠르게 문서를 포맷팅할 수 있는
아주 유용한 도구입니다.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_wrap_lists() {
        let input = "  - This is a very long list item line that should be nicely wrapped to multiple lines preserving the bullet indentation.";
        let options = FormatOptions {
            wrap_lists: true,
            fill_column: 40,
            ..FormatOptions::default()
        };
        let expected = "  - This is a very long list item line
    that should be nicely wrapped to
    multiple lines preserving the bullet
    indentation.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_wrap_lists_korean() {
        let input = "  * 매우 긴 한글 리스트 아이템입니다. 이것은 지정된 너비에 맞추어 여러 줄로 깔끔하게 래핑되어야 합니다.";
        let options = FormatOptions {
            wrap_lists: true,
            fill_column: 40,
            ..FormatOptions::default()
        };
        let expected = "  * 매우 긴 한글 리스트 아이템입니다.
    이것은 지정된 너비에 맞추어 여러
    줄로 깔끔하게 래핑되어야 합니다.
";
        assert_eq!(format_org(input, &options), expected);
    }

    #[test]
    fn test_wrap_lists_ordered() {
        let input = " 1. First ordered item that spans a considerable length to test the ordered list wrapping.";
        let options = FormatOptions {
            wrap_lists: true,
            fill_column: 40,
            ..FormatOptions::default()
        };
        let expected = " 1. First ordered item that spans a
    considerable length to test the
    ordered list wrapping.
";
        assert_eq!(format_org(input, &options), expected);
    }
}
