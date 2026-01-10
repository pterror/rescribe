//! Handwritten LaTeX parser.

use rescribe_core::{
    ConversionResult, Document, FidelityWarning, ParseError, ParseOptions, Severity, Span,
    WarningKind,
};
use rescribe_std::{Node, node, prop};

/// Parse LaTeX text into a rescribe Document.
pub fn parse(input: &str) -> Result<ConversionResult<Document>, ParseError> {
    parse_with_options(input, &ParseOptions::default())
}

/// Parse LaTeX with custom options.
pub fn parse_with_options(
    input: &str,
    options: &ParseOptions,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut parser = Parser::new(input, options.preserve_source_info);
    let (children, warnings) = parser.parse_document();

    let root = Node::new(node::DOCUMENT).children(children);
    let doc = Document::new().with_content(root);

    Ok(ConversionResult::with_warnings(doc, warnings))
}

/// LaTeX parser state.
struct Parser<'a> {
    input: &'a str,
    pos: usize,
    warnings: Vec<FidelityWarning>,
    preserve_spans: bool,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, preserve_spans: bool) -> Self {
        Self {
            input,
            pos: 0,
            warnings: Vec::new(),
            preserve_spans,
        }
    }

    /// Create a span from start to current position.
    fn make_span(&self, start: usize) -> Option<Span> {
        if self.preserve_spans {
            Some(Span {
                start,
                end: self.pos,
            })
        } else {
            None
        }
    }

    /// Add span to node if preserving spans.
    fn with_span(&self, mut node: Node, start: usize) -> Node {
        node.span = self.make_span(start);
        node
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.input.len());
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance(c.len_utf8());
            } else {
                break;
            }
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        self.remaining().starts_with(s)
    }

    fn parse_document(&mut self) -> (Vec<Node>, Vec<FidelityWarning>) {
        // Skip preamble (everything before \begin{document} if present)
        if let Some(idx) = self.input.find("\\begin{document}") {
            self.pos = idx + "\\begin{document}".len();
        }

        // Skip leading whitespace
        self.skip_whitespace();

        let mut children = Vec::new();
        let mut current_para = Vec::new();

        while !self.is_eof() {
            // Check for end of document
            if self.starts_with("\\end{document}") {
                break;
            }

            // Skip comments
            if self.starts_with("%") {
                self.skip_line();
                continue;
            }

            // Check for blank line (paragraph break)
            if self.check_blank_line() {
                if !current_para.is_empty() {
                    children.push(
                        Node::new(node::PARAGRAPH).children(std::mem::take(&mut current_para)),
                    );
                }
                self.skip_blank_lines();
                continue;
            }

            // Try to parse a block element
            if let Some(block) = self.try_parse_block() {
                if !current_para.is_empty() {
                    children.push(
                        Node::new(node::PARAGRAPH).children(std::mem::take(&mut current_para)),
                    );
                }
                children.push(block);
                continue;
            }

            // Parse inline content
            if let Some(inline) = self.parse_inline() {
                current_para.push(inline);
            } else {
                // Consume one character to prevent infinite loop
                self.advance(1);
            }
        }

        // Flush remaining paragraph
        if !current_para.is_empty() {
            children.push(Node::new(node::PARAGRAPH).children(current_para));
        }

        (children, std::mem::take(&mut self.warnings))
    }

    fn check_blank_line(&self) -> bool {
        let remaining = self.remaining();
        let mut newline_count = 0;

        for c in remaining.chars() {
            if c == '\n' {
                newline_count += 1;
                if newline_count >= 2 {
                    return true;
                }
            } else if !c.is_whitespace() {
                return false;
            }
        }
        false
    }

    fn skip_blank_lines(&mut self) {
        while !self.is_eof() {
            let c = self.peek_char().unwrap();
            if c.is_whitespace() {
                self.advance(c.len_utf8());
            } else {
                break;
            }
        }
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.peek_char() {
            self.advance(c.len_utf8());
            if c == '\n' {
                break;
            }
        }
    }

    fn try_parse_block(&mut self) -> Option<Node> {
        if !self.starts_with("\\") {
            return None;
        }

        let start = self.pos;

        // Section commands
        if self.starts_with("\\section{") {
            return Some(self.parse_section(1, start));
        }
        if self.starts_with("\\subsection{") {
            return Some(self.parse_section(2, start));
        }
        if self.starts_with("\\subsubsection{") {
            return Some(self.parse_section(3, start));
        }
        if self.starts_with("\\paragraph{") {
            return Some(self.parse_section(4, start));
        }

        // Environments
        if self.starts_with("\\begin{") {
            return self.parse_environment(start);
        }

        // Horizontal rule
        if self.starts_with("\\hrule") || self.starts_with("\\rule{") {
            self.skip_command();
            return Some(self.with_span(Node::new(node::HORIZONTAL_RULE), start));
        }

        None
    }

    fn parse_section(&mut self, level: i64, start: usize) -> Node {
        // Skip the command name
        self.skip_until('{');
        self.advance(1); // skip '{'

        let content = self.parse_until_closing_brace();
        let children = self.parse_inline_content(&content);

        self.with_span(
            Node::new(node::HEADING)
                .prop(prop::LEVEL, level)
                .children(children),
            start,
        )
    }

    fn parse_environment(&mut self, start: usize) -> Option<Node> {
        self.advance("\\begin{".len());

        let env_name = self.parse_until('}');
        self.advance(1); // skip '}'

        match env_name.as_str() {
            "itemize" => Some(self.parse_list(false, start)),
            "enumerate" => Some(self.parse_list(true, start)),
            "verbatim" | "lstlisting" => Some(self.parse_verbatim(&env_name, start)),
            "quote" | "quotation" => Some(self.parse_blockquote(&env_name, start)),
            "equation" | "equation*" | "align" | "align*" => {
                Some(self.parse_math_env(&env_name, start))
            }
            "figure" => Some(self.parse_figure(start)),
            "table" => Some(self.parse_table_env(start)),
            "tabular" => Some(self.parse_tabular(start)),
            _ => {
                self.warnings.push(FidelityWarning::new(
                    Severity::Minor,
                    WarningKind::UnsupportedNode(format!("latex:{}", env_name)),
                    format!("Unknown environment: {}", env_name),
                ));
                // Skip to end of environment
                self.skip_environment(&env_name);
                None
            }
        }
    }

    fn parse_list(&mut self, ordered: bool, start: usize) -> Node {
        let env_name = if ordered { "enumerate" } else { "itemize" };
        let mut items = Vec::new();

        while !self.is_eof() {
            self.skip_whitespace();

            if self.starts_with(&format!("\\end{{{}}}", env_name)) {
                self.advance(format!("\\end{{{}}}", env_name).len());
                break;
            }

            if self.starts_with("\\item") {
                let item_start = self.pos;
                self.advance("\\item".len());
                // Skip optional [label]
                if self.starts_with("[") {
                    self.skip_until(']');
                    self.advance(1);
                }

                let content = self.parse_item_content(env_name);
                items
                    .push(self.with_span(Node::new(node::LIST_ITEM).children(content), item_start));
            } else if self.starts_with("%") {
                self.skip_line();
            } else {
                self.advance(1);
            }
        }

        self.with_span(
            Node::new(node::LIST)
                .prop(prop::ORDERED, ordered)
                .children(items),
            start,
        )
    }

    fn parse_item_content(&mut self, env_name: &str) -> Vec<Node> {
        let mut content = Vec::new();
        let mut text = String::new();

        while !self.is_eof() {
            if self.starts_with("\\item") || self.starts_with(&format!("\\end{{{}}}", env_name)) {
                break;
            }

            if self.starts_with("\\begin{") {
                if !text.is_empty() {
                    content.extend(self.parse_inline_content(&text));
                    text.clear();
                }
                let env_start = self.pos;
                if let Some(block) = self.parse_environment(env_start) {
                    content.push(block);
                }
            } else if self.starts_with("\\") {
                // Try to parse inline command
                if let Some(inline) = self.parse_inline() {
                    if !text.is_empty() {
                        content.extend(self.parse_inline_content(&text));
                        text.clear();
                    }
                    content.push(inline);
                } else if let Some(c) = self.peek_char() {
                    text.push(c);
                    self.advance(c.len_utf8());
                }
            } else if let Some(c) = self.peek_char() {
                text.push(c);
                self.advance(c.len_utf8());
            }
        }

        if !text.is_empty() {
            content.extend(self.parse_inline_content(&text));
        }

        content
    }

    fn parse_verbatim(&mut self, env_name: &str, start: usize) -> Node {
        let end_tag = format!("\\end{{{}}}", env_name);
        let mut content = String::new();

        while !self.is_eof() {
            if self.starts_with(&end_tag) {
                self.advance(end_tag.len());
                break;
            }
            if let Some(c) = self.peek_char() {
                content.push(c);
                self.advance(c.len_utf8());
            }
        }

        // Trim leading/trailing newlines
        let content = content.trim_matches('\n').to_string();

        self.with_span(
            Node::new(node::CODE_BLOCK).prop(prop::CONTENT, content),
            start,
        )
    }

    fn parse_blockquote(&mut self, env_name: &str, start: usize) -> Node {
        let end_tag = format!("\\end{{{}}}", env_name);
        let mut content = String::new();

        while !self.is_eof() {
            if self.starts_with(&end_tag) {
                self.advance(end_tag.len());
                break;
            }
            if let Some(c) = self.peek_char() {
                content.push(c);
                self.advance(c.len_utf8());
            }
        }

        let children = self.parse_inline_content(content.trim());

        self.with_span(
            Node::new(node::BLOCKQUOTE)
                .children(vec![Node::new(node::PARAGRAPH).children(children)]),
            start,
        )
    }

    fn parse_math_env(&mut self, env_name: &str, start: usize) -> Node {
        let end_tag = format!("\\end{{{}}}", env_name);
        let mut content = String::new();

        while !self.is_eof() {
            if self.starts_with(&end_tag) {
                self.advance(end_tag.len());
                break;
            }
            if let Some(c) = self.peek_char() {
                content.push(c);
                self.advance(c.len_utf8());
            }
        }

        self.with_span(
            Node::new("math_display").prop("math:source", content.trim().to_string()),
            start,
        )
    }

    fn parse_figure(&mut self, start: usize) -> Node {
        let mut children = Vec::new();
        let end_tag = "\\end{figure}";

        while !self.is_eof() {
            self.skip_whitespace();

            if self.starts_with(end_tag) {
                self.advance(end_tag.len());
                break;
            }

            if self.starts_with("\\includegraphics") {
                let img_start = self.pos;
                if let Some(img) = self.parse_includegraphics(img_start) {
                    children.push(img);
                }
            } else if self.starts_with("\\caption{") {
                let caption_start = self.pos;
                self.advance("\\caption{".len());
                let caption_text = self.parse_until_closing_brace();
                let caption_children = self.parse_inline_content(&caption_text);
                children.push(self.with_span(
                    Node::new(node::CAPTION).children(caption_children),
                    caption_start,
                ));
            } else if self.starts_with("%") {
                self.skip_line();
            } else {
                self.advance(1);
            }
        }

        self.with_span(Node::new(node::FIGURE).children(children), start)
    }

    fn parse_includegraphics(&mut self, start: usize) -> Option<Node> {
        self.advance("\\includegraphics".len());

        // Skip optional [options]
        if self.starts_with("[") {
            self.skip_until(']');
            self.advance(1);
        }

        if !self.starts_with("{") {
            return None;
        }
        self.advance(1);

        let path = self.parse_until_closing_brace();

        Some(self.with_span(Node::new(node::IMAGE).prop(prop::URL, path), start))
    }

    fn parse_table_env(&mut self, start: usize) -> Node {
        // Just find and parse the tabular inside
        while !self.is_eof() {
            if self.starts_with("\\end{table}") {
                self.advance("\\end{table}".len());
                break;
            }
            if self.starts_with("\\begin{tabular}") {
                let tabular_start = self.pos;
                return self
                    .parse_environment(tabular_start)
                    .unwrap_or_else(|| self.with_span(Node::new(node::TABLE), start));
            }
            self.advance(1);
        }
        self.with_span(Node::new(node::TABLE), start)
    }

    fn parse_tabular(&mut self, start: usize) -> Node {
        // Skip column spec
        if self.starts_with("{") {
            self.skip_until('}');
            self.advance(1);
        }

        let mut rows = Vec::new();
        let mut current_row = Vec::new();
        let mut current_cell = String::new();

        while !self.is_eof() {
            if self.starts_with("\\end{tabular}") {
                self.advance("\\end{tabular}".len());
                break;
            }

            if self.starts_with("\\\\") {
                // End of row
                if !current_cell.is_empty() || !current_row.is_empty() {
                    let cell_content = self.parse_inline_content(current_cell.trim());
                    current_row.push(Node::new(node::TABLE_CELL).children(cell_content));
                    current_cell.clear();
                }
                if !current_row.is_empty() {
                    rows.push(Node::new(node::TABLE_ROW).children(current_row));
                    current_row = Vec::new();
                }
                self.advance(2);
                continue;
            }

            if self.starts_with("&") {
                // Cell separator
                let cell_content = self.parse_inline_content(current_cell.trim());
                current_row.push(Node::new(node::TABLE_CELL).children(cell_content));
                current_cell.clear();
                self.advance(1);
                continue;
            }

            if self.starts_with("\\hline") {
                self.advance("\\hline".len());
                continue;
            }

            if let Some(c) = self.peek_char() {
                current_cell.push(c);
                self.advance(c.len_utf8());
            }
        }

        // Flush last row
        if !current_cell.is_empty() {
            let cell_content = self.parse_inline_content(current_cell.trim());
            current_row.push(Node::new(node::TABLE_CELL).children(cell_content));
        }
        if !current_row.is_empty() {
            rows.push(Node::new(node::TABLE_ROW).children(current_row));
        }

        self.with_span(Node::new(node::TABLE).children(rows), start)
    }

    fn skip_environment(&mut self, env_name: &str) {
        let end_tag = format!("\\end{{{}}}", env_name);
        while !self.is_eof() {
            if self.starts_with(&end_tag) {
                self.advance(end_tag.len());
                break;
            }
            self.advance(1);
        }
    }

    fn skip_command(&mut self) {
        // Skip \command or \command{...} or \command[...]{...}
        self.advance(1); // skip '\'
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() {
                self.advance(c.len_utf8());
            } else {
                break;
            }
        }
        // Skip optional arguments
        while self.starts_with("[") {
            self.skip_until(']');
            self.advance(1);
        }
        while self.starts_with("{") {
            self.skip_until('}');
            self.advance(1);
        }
    }

    fn skip_until(&mut self, target: char) {
        while let Some(c) = self.peek_char() {
            if c == target {
                break;
            }
            self.advance(c.len_utf8());
        }
    }

    fn parse_until(&mut self, target: char) -> String {
        let mut result = String::new();
        while let Some(c) = self.peek_char() {
            if c == target {
                break;
            }
            result.push(c);
            self.advance(c.len_utf8());
        }
        result
    }

    fn parse_until_closing_brace(&mut self) -> String {
        let mut result = String::new();
        let mut depth = 1;

        while let Some(c) = self.peek_char() {
            match c {
                '{' => {
                    depth += 1;
                    result.push(c);
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        self.advance(1);
                        break;
                    }
                    result.push(c);
                }
                _ => result.push(c),
            }
            self.advance(c.len_utf8());
        }

        result
    }

    fn parse_inline(&mut self) -> Option<Node> {
        if !self.starts_with("\\") {
            // Check for inline math
            if self.starts_with("$") && !self.starts_with("$$") {
                return Some(self.parse_inline_math());
            }
            if self.starts_with("$$") {
                return Some(self.parse_display_math_dollar());
            }

            // Regular text
            return self.parse_text();
        }

        // Try various inline commands
        if self.starts_with("\\textbf{") || self.starts_with("\\bf{") {
            return Some(self.parse_inline_command(node::STRONG));
        }
        if self.starts_with("\\textit{") || self.starts_with("\\it{") || self.starts_with("\\emph{")
        {
            return Some(self.parse_inline_command(node::EMPHASIS));
        }
        if self.starts_with("\\texttt{") || self.starts_with("\\verb") {
            return Some(self.parse_code());
        }
        if self.starts_with("\\underline{") {
            return Some(self.parse_inline_command(node::UNDERLINE));
        }
        if self.starts_with("\\sout{") || self.starts_with("\\st{") {
            return Some(self.parse_inline_command(node::STRIKEOUT));
        }
        if self.starts_with("\\textsc{") {
            return Some(self.parse_inline_command(node::SMALL_CAPS));
        }
        if self.starts_with("\\textsuperscript{") || self.starts_with("\\textsup{") {
            return Some(self.parse_inline_command(node::SUPERSCRIPT));
        }
        if self.starts_with("\\textsubscript{") || self.starts_with("\\textsub{") {
            return Some(self.parse_inline_command(node::SUBSCRIPT));
        }
        if self.starts_with("\\href{") {
            return Some(self.parse_href());
        }
        if self.starts_with("\\url{") {
            return Some(self.parse_url());
        }
        if self.starts_with("\\(") {
            return Some(self.parse_inline_math_paren());
        }
        if self.starts_with("\\[") {
            return Some(self.parse_display_math_bracket());
        }

        // Escaped characters
        if self.starts_with("\\\\") {
            self.advance(2);
            return Some(Node::new(node::LINE_BREAK));
        }
        if self.starts_with("\\&")
            || self.starts_with("\\%")
            || self.starts_with("\\$")
            || self.starts_with("\\_")
            || self.starts_with("\\#")
            || self.starts_with("\\{")
            || self.starts_with("\\}")
        {
            let c = self.remaining().chars().nth(1).unwrap();
            self.advance(2);
            return Some(Node::new(node::TEXT).prop(prop::CONTENT, c.to_string()));
        }

        // Unknown command - try to skip it
        None
    }

    fn parse_inline_command(&mut self, kind: &str) -> Node {
        self.skip_until('{');
        self.advance(1);
        let content = self.parse_until_closing_brace();
        let children = self.parse_inline_content(&content);
        Node::new(kind).children(children)
    }

    fn parse_code(&mut self) -> Node {
        if self.starts_with("\\verb") {
            self.advance("\\verb".len());
            // \verb|code| - delimiter is the next character
            if let Some(delim) = self.peek_char() {
                self.advance(delim.len_utf8());
                let content = self.parse_until(delim);
                self.advance(delim.len_utf8());
                return Node::new(node::CODE).prop(prop::CONTENT, content);
            }
        }
        // \texttt{code}
        self.skip_until('{');
        self.advance(1);
        let content = self.parse_until_closing_brace();
        Node::new(node::CODE).prop(prop::CONTENT, content)
    }

    fn parse_href(&mut self) -> Node {
        self.advance("\\href{".len());
        let url = self.parse_until_closing_brace();
        if self.starts_with("{") {
            self.advance(1);
            let text = self.parse_until_closing_brace();
            let children = self.parse_inline_content(&text);
            Node::new(node::LINK)
                .prop(prop::URL, url)
                .children(children)
        } else {
            Node::new(node::LINK)
                .prop(prop::URL, url.clone())
                .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, url)])
        }
    }

    fn parse_url(&mut self) -> Node {
        self.advance("\\url{".len());
        let url = self.parse_until_closing_brace();
        Node::new(node::LINK)
            .prop(prop::URL, url.clone())
            .children(vec![Node::new(node::TEXT).prop(prop::CONTENT, url)])
    }

    fn parse_inline_math(&mut self) -> Node {
        self.advance(1); // skip $
        let mut content = String::new();
        while let Some(c) = self.peek_char() {
            if c == '$' {
                self.advance(1);
                break;
            }
            content.push(c);
            self.advance(c.len_utf8());
        }
        Node::new("math_inline").prop("math:source", content)
    }

    fn parse_inline_math_paren(&mut self) -> Node {
        self.advance(2); // skip \(
        let mut content = String::new();
        while !self.is_eof() {
            if self.starts_with("\\)") {
                self.advance(2);
                break;
            }
            if let Some(c) = self.peek_char() {
                content.push(c);
                self.advance(c.len_utf8());
            }
        }
        Node::new("math_inline").prop("math:source", content)
    }

    fn parse_display_math_dollar(&mut self) -> Node {
        self.advance(2); // skip $$
        let mut content = String::new();
        while !self.is_eof() {
            if self.starts_with("$$") {
                self.advance(2);
                break;
            }
            if let Some(c) = self.peek_char() {
                content.push(c);
                self.advance(c.len_utf8());
            }
        }
        Node::new("math_display").prop("math:source", content)
    }

    fn parse_display_math_bracket(&mut self) -> Node {
        self.advance(2); // skip \[
        let mut content = String::new();
        while !self.is_eof() {
            if self.starts_with("\\]") {
                self.advance(2);
                break;
            }
            if let Some(c) = self.peek_char() {
                content.push(c);
                self.advance(c.len_utf8());
            }
        }
        Node::new("math_display").prop("math:source", content)
    }

    fn parse_text(&mut self) -> Option<Node> {
        let mut text = String::new();

        while let Some(c) = self.peek_char() {
            // Stop at special characters
            if c == '\\' || c == '$' || c == '{' || c == '}' || c == '&' || c == '%' || c == '\n' {
                break;
            }
            text.push(c);
            self.advance(c.len_utf8());
        }

        if text.is_empty() {
            // Handle single newline (soft break)
            if self.peek_char() == Some('\n') {
                self.advance(1);
                return Some(Node::new(node::SOFT_BREAK));
            }
            return None;
        }

        Some(Node::new(node::TEXT).prop(prop::CONTENT, text))
    }

    fn parse_inline_content(&self, content: &str) -> Vec<Node> {
        let mut parser = Parser::new(content, self.preserve_spans);
        let mut nodes = Vec::new();

        while !parser.is_eof() {
            if let Some(node) = parser.parse_inline() {
                nodes.push(node);
            } else {
                parser.advance(1);
            }
        }

        // Merge adjacent text nodes
        merge_text_nodes(&mut nodes);

        nodes
    }
}

/// Merge adjacent text nodes.
fn merge_text_nodes(nodes: &mut Vec<Node>) {
    let mut i = 0;
    while i + 1 < nodes.len() {
        if nodes[i].kind.as_str() == node::TEXT && nodes[i + 1].kind.as_str() == node::TEXT {
            let next_content = nodes[i + 1]
                .props
                .get_str(prop::CONTENT)
                .unwrap_or("")
                .to_string();
            let current_content = nodes[i]
                .props
                .get_str(prop::CONTENT)
                .unwrap_or("")
                .to_string();

            nodes[i] = Node::new(node::TEXT).prop(prop::CONTENT, current_content + &next_content);
            nodes.remove(i + 1);
        } else {
            i += 1;
        }
    }
}
