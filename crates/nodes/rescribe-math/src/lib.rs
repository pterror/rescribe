//! Math node kinds for rescribe.
//!
//! This crate provides node kinds for mathematical notation,
//! supporting both presentation and semantic math.

pub use rescribe_core::*;

/// Math node kind constants.
pub mod node {
    /// Inline math expression.
    pub const MATH_INLINE: &str = "math_inline";
    /// Display/block math expression.
    pub const MATH_DISPLAY: &str = "math_display";

    // Structural elements
    /// A fraction (numerator/denominator).
    pub const FRACTION: &str = "math:fraction";
    /// A square root or nth root.
    pub const ROOT: &str = "math:root";
    /// Subscript in math context.
    pub const MATH_SUB: &str = "math:sub";
    /// Superscript in math context.
    pub const MATH_SUP: &str = "math:sup";
    /// Combined sub/superscript.
    pub const MATH_SUBSUP: &str = "math:subsup";
    /// Underscript (like limits below).
    pub const MATH_UNDER: &str = "math:under";
    /// Overscript (like limits above).
    pub const MATH_OVER: &str = "math:over";
    /// Combined under/overscript.
    pub const MATH_UNDEROVER: &str = "math:underover";

    // Containers
    /// A matrix or array.
    pub const MATRIX: &str = "math:matrix";
    /// A row in a matrix.
    pub const MATRIX_ROW: &str = "math:matrix_row";
    /// A cell in a matrix row.
    pub const MATRIX_CELL: &str = "math:matrix_cell";
    /// Parenthesized/bracketed expression.
    pub const FENCED: &str = "math:fenced";
    /// A table/aligned environment.
    pub const MATH_TABLE: &str = "math:table";

    // Semantic elements
    /// An operator (+, -, ×, etc.).
    pub const OPERATOR: &str = "math:operator";
    /// An identifier (variable name).
    pub const IDENTIFIER: &str = "math:identifier";
    /// A number.
    pub const NUMBER: &str = "math:number";
    /// Text within math.
    pub const MATH_TEXT: &str = "math:text";
    /// A mathematical space.
    pub const MATH_SPACE: &str = "math:space";

    // Decorations
    /// Accent over expression (hat, tilde, etc.).
    pub const ACCENT: &str = "math:accent";
    /// Horizontal brace/bracket over/under.
    pub const MATH_BRACE: &str = "math:brace";
    /// Struck-through expression.
    pub const MATH_STRIKE: &str = "math:strike";
    /// Boxed/enclosed expression.
    pub const ENCLOSED: &str = "math:enclosed";
}

/// Math property key constants.
pub mod prop {
    /// The math format (latex, mathml, asciimath).
    pub const MATH_FORMAT: &str = "math:format";
    /// Raw math source text.
    pub const MATH_SOURCE: &str = "math:source";
    /// Root index for nth roots.
    pub const ROOT_INDEX: &str = "math:root_index";
    /// Opening delimiter for fenced expressions.
    pub const OPEN_DELIM: &str = "math:open";
    /// Closing delimiter for fenced expressions.
    pub const CLOSE_DELIM: &str = "math:close";
    /// Accent character (hat, tilde, etc.).
    pub const ACCENT_CHAR: &str = "math:accent_char";
    /// Whether accent stretches over content.
    pub const ACCENT_STRETCHY: &str = "math:accent_stretchy";
    /// Operator form (prefix, infix, postfix).
    pub const OPERATOR_FORM: &str = "math:op_form";
    /// Whether operator is a large operator.
    pub const LARGE_OP: &str = "math:large_op";
    /// Whether to move limits.
    pub const MOVE_LIMITS: &str = "math:move_limits";
    /// Matrix/table alignment.
    pub const COLUMN_ALIGN: &str = "math:column_align";
    /// Row alignment.
    pub const ROW_ALIGN: &str = "math:row_align";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_math_inline() {
        let math = Node::new(node::MATH_INLINE)
            .prop(prop::MATH_FORMAT, "latex")
            .prop(prop::MATH_SOURCE, "x^2 + y^2 = z^2");

        assert_eq!(math.kind.as_str(), "math_inline");
        assert_eq!(math.props.get_str(prop::MATH_FORMAT), Some("latex"));
    }
}
